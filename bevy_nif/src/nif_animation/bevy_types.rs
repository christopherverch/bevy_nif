// src/nif_animation/bevy_types.rs

use std::collections::HashMap;

use bevy::ecs::entity::Entity;
use bevy::prelude::*;
use bitflags::bitflags;
use serde::{Deserialize, Serialize};

// Forward declare Skeleton if it's in another module (e.g., crate::nif::skeleton::Skeleton)
// For this file, we'll assume it's accessible. If not, adjust the path.
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
    pub node_index: AnimationNodeIndex, // Bevy AnimationGraph node index for the clip
    /// The handle to the Bevy AnimationClip asset being played.
    pub clip_handle: Handle<AnimationClip>,
    pub next_clip_name: Option<String>, // The clip to play after this animation finishes, likely a
    // loop such as RunForward_loop
    pub duration: f32,
    /// The intrinsic velocity of this animation, calculated from the root bone's movement.
    /// This is used to sync physics speed with animation speed.
    pub base_velocity: Vec3,
    /// The isolated translation curve for the root bone (`Bip01`).
    /// This is sampled manually for root motion.
    pub root_translation_curve: Option<AnimatableKeyframeCurve<Vec3>>,
    pub min_attack_time_relative: f32, // For attack animations only
    pub hit_time_relative: f32,
    pub min_hit_time_relative: f32,
}
#[derive(Debug, Clone)]
pub struct ActiveAnimation {
    /// The handle to the Bevy AnimationClip asset being played.
    pub clip_handle: Handle<AnimationClip>,
    pub node_index: AnimationNodeIndex,
    /// How many times this animation should loop. u32::MAX can represent indefinite looping.
    pub loop_count: u32,
    /// A bitmask defining which body parts this animation affects.
    pub blend_mask: BlendMask,
    /// The name of the clip to transition to when this one finishes.
    pub next_clip_name: Option<String>,
    pub priorities: [Priority; NUM_DISCRETE_REGIONS],
    pub speed_mult: f32,
}
/// Defines the animation priority levels, ordered from lowest to highest.
///
/// This enum directly corresponds to `MWMechanics::Priority` from the OpenMW source.
/// The order of variants is critical, as it determines which animations override others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Priority {
    /// The default, lowest-priority state, typically for idle animations.
    #[default]
    Idle, // Renamed from "Default" for clarity.
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
    /// The absolute highest priority, used for scripted sequences that must not be interrupted.
    Scripted,
}

#[derive(Component)]
pub struct NifAnimator {
    pub skeleton_id: u64,
    // Maps canonical animation name (e.g., "Idle", "HandToHand:Chop") to its definition
    pub animation_definitions: HashMap<String, AnimationDefinition>,
    // Currently playing animations on this animator, keyed by a unique instance ID or canonical name
    pub active_animations: HashMap<String, ActiveAnimation>,
    // Which priorities are currently controlling the regions
    pub active_regions: [Priority; NUM_DISCRETE_REGIONS],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Event)]
pub enum NifEventType {
    SoundGen { sound_name: String },
}
#[derive(Clone, Debug, Event, Serialize, Deserialize)]
pub struct NifEvent {
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

// Constants for region determination (can also live here or in a config module)
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
