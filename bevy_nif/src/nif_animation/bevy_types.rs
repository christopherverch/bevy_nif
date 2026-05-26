use bevy::prelude::*;
use bevy::{animation::AnimationEvent, ecs::entity::Entity};
use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::skeleton::Skeleton;

#[derive(Resource, Debug, Default)]
pub struct SkeletonMap {
    /// Maps skeleton id to entity
    pub root_skeleton_entity_map: HashMap<u64, Entity>,
    /// Maps skeleton id to the Skeleton
    pub skeletons: HashMap<u64, Skeleton>,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)] // Added Default
    pub struct BlendMask: u64 {         const NONE       = 0b00000000;
        const LOWER_BODY = 1 << 0;    // 0b00000001
        const TORSO      = 1 << 1;    // 0b00000010
        const LEFT_ARM   = 1 << 2;    // 0b00000100
        const RIGHT_ARM  = 1 << 3;    // 0b00001000

        const UPPER_BODY = Self::TORSO.bits() | Self::LEFT_ARM.bits() | Self::RIGHT_ARM.bits();         const ALL        = Self::LOWER_BODY.bits() | Self::UPPER_BODY.bits();     }
}
#[derive(Debug, Clone)]
pub struct AnimationDefinition {
    /// Bevy AnimationGraph node index for the clip
    pub node_index: AnimationNodeIndex,
    /// The handle to the Bevy AnimationClip asset being played.
    pub clip_handle: Handle<AnimationClip>,
    /// The clip to play after this animation finishes, likely a
    /// loop such as RunForward_loop
    pub next_clip_name: Option<String>,
    pub duration: f32,
    /// The intrinsic velocity of this animation, calculated from the root bone's movement.
    /// This is used to sync physics speed with animation speed.
    pub base_velocity: Vec3,
    /// The isolated translation curve for the root bone (`Bip01`).
    /// This is sampled manually for root motion.
    pub root_translation_curve: Option<AnimatableKeyframeCurve<Vec3>>,
    pub animation_events: Vec<(f32, ManualNifEvent)>,
    /// For attack animations only
    pub min_attack_time_relative: f32,
    /// For attack animations only
    pub hit_time_relative: f32,
    /// For attack animations only
    pub min_hit_time_relative: f32,
}
#[derive(Debug, Clone)]
pub struct ActiveAnimation {
    /// Bevy AnimationGraph node index for the clip
    pub node_index: AnimationNodeIndex,
    /// The handle to the Bevy AnimationClip asset being played.
    pub clip_handle: Handle<AnimationClip>,
    /// How many times this animation should loop
    pub loop_count: u32,
    /// A bitmask defining which body parts this animation affects.
    pub blend_mask: BlendMask,
    /// The name of the clip to transition to when this one finishes.
    pub next_clip_name: Option<String>,
    /// Priorities for each region for this animation
    pub priorities: [Priority; NUM_DISCRETE_REGIONS],
    /// Whether this animation should be removed when finished, or just freeze on the last frame
    /// Useful for things like a jump animation freezing on the last frame
    pub auto_remove: bool,
    pub speed_mult: f32,
}
/// Defines the animation priority levels, ordered from lowest to highest.
/// determines which animations override others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Priority {
    #[default]
    Default,
    WeaponLowerBody,
    SneakIdleLowerBody,
    SwimIdle,
    Jump,
    Movement,
    Hit,
    Weapon,
    Block,
    Knockdown,
    Torch,
    Storm,
    Death,
    Scripted,
}

#[derive(Component)]
pub struct NifAnimator {
    pub skeleton_id: u64,
    // Maps animation name (e.g., "Idle", "HandToHand:Chop") to its definition
    pub animation_definitions: HashMap<String, AnimationDefinition>,
    // Currently playing animations on this animator, keyed by name
    pub active_animations: HashMap<String, ActiveAnimation>,
    // Which priorities are currently controlling which regions
    pub active_regions: [Priority; NUM_DISCRETE_REGIONS],
}
impl NifAnimator {
    /// Returns true if the animation is finished, or not found
    pub fn is_finished(
        clip_handle: Handle<AnimationClip>,
        anim_clips: &Assets<AnimationClip>,
        anim_state: &bevy::animation::ActiveAnimation,
    ) -> bool {
        // Since `seek_time()` is the current progress, we just need to
        // check if it has reached the end of the clip.
        let Some(clip) = anim_clips.get(&clip_handle) else {
            return false;
        };
        let total_duration = clip.duration();
        if anim_state.seek_time() >= total_duration - 0.001 {
            return true;
        } else {
            return false;
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NifEventType {
    SoundGen { sound_name: String },
}
#[derive(Clone, Debug, Serialize, Deserialize, AnimationEvent)]
pub struct NifEvent {
    pub entity: Entity,
    pub event_type: NifEventType,
}
/// So we can manually trigger events that would normally be animation events
#[derive(Clone, Debug, Serialize, Deserialize, EntityEvent)]
pub struct ManualNifEvent {
    pub entity: Entity,
    pub event_type: NifEventType,
}
#[derive(Event)]
pub struct NifAnimatorAdded(pub Entity);

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub enum AnimationTransitionState {
    #[default]
    Sustained,
    FadingIn {
        total_duration: f32,
        elapsed_time: f32,
    },
    FadingOut {
        total_duration: f32,
        elapsed_time: f32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum AnimationRepeatBehavior {
    #[default]
    PlayOnce,
    LoopIndefinitely,
    LoopCount(u32),
}

pub const REGION_ROOT_LOWER_BODY: &str = "Bip01";
pub const REGION_ROOT_LEFT_LEG: &str = "Bip01 L Thigh";
pub const REGION_ROOT_RIGHT_LEG: &str = "Bip01 R Thigh";
pub const REGION_ROOT_TORSO: &str = "Bip01 Spine1";
pub const REGION_ROOT_LEFT_ARM: &str = "Bip01 L Clavicle";
pub const REGION_ROOT_RIGHT_ARM: &str = "Bip01 R Clavicle";
pub const REGION_INDEX_LOWER_BODY: usize = 0;
pub const REGION_INDEX_TORSO: usize = 1;
pub const REGION_INDEX_LEFT_ARM: usize = 2;
pub const REGION_INDEX_RIGHT_ARM: usize = 3;
pub const NUM_DISCRETE_REGIONS: usize = 4;
