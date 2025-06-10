// src/nif_animation/bevy_types.rs

use std::collections::HashMap;

use bevy::ecs::entity::Entity;
use bevy::prelude::*;
use bitflags::bitflags;
use serde::{Deserialize, Serialize};

// Forward declare Skeleton if it's in another module (e.g., crate::nif::skeleton::Skeleton)
// For this file, we'll assume it's accessible. If not, adjust the path.
use crate::nif::skeleton::Skeleton;

#[derive(Resource, Debug, Default)]
pub struct SkeletonMap {
    pub root_skeleton_entity_map: HashMap<u64, Entity>,
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

#[derive(Debug, Clone)] // Removed Default as node_index might not have a sensible default
pub struct AnimationDefinition {
    pub node_index: AnimationNodeIndex, // Bevy AnimationGraph node index for the clip
    pub inherent_mask: BlendMask,       // The combined mask of all regions this animation affects
                                        // Consider adding:
                                        // pub clip_handle: Handle<AnimationClip>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)] // Added Default
pub struct ActiveAnimNif {
    pub animation_node_index: AnimationNodeIndex, // In the NifAnimator's graph
    pub name: String, // For identification, debugging, and fetching defaults
    pub priority: u8, // Or consider [u8; 4] for per-region priority
    pub mask: BlendMask, // The runtime mask this instance is playing on
                      // pub transition_state: AnimationTransitionState, // You had this, likely useful
                      // pub repeat_behavior: AnimationRepeatBehavior, // You had this
                      // pub current_time: f32,
                      // pub speed_multiplier: f32,
}

#[derive(Component)]
pub struct NifAnimator {
    pub skeleton_id: u64,
    // Maps canonical animation name (e.g., "Idle", "HandToHand:Chop") to its definition
    pub animation_definitions: HashMap<String, AnimationDefinition>,
    // Currently playing animations on this animator, keyed by a unique instance ID or canonical name
    pub active_animations: HashMap<String, ActiveAnimNif>, // Maps bone names to their primary region index (0-3 for LowerBody, Torso, LeftArm, RightArm)
    pub bone_to_region_index_map: HashMap<String, usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Resource)] // Eq requires PartialEq
pub enum NifEventType {
    LoopAnimation { animation_name: String },
    FreezeAnimation { animation_name: String },
    ResumeIdle { animation_name: String },
    Other { event: String },
}

#[derive(Clone, Debug, Event, Serialize, Deserialize)] // Removed Resource, Event is more typical for NifEvent
pub struct NifEvent {
    pub skeleton_id: u64,
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
