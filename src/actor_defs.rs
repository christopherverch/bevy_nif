#![allow(dead_code)]
// This file defines the Rust structs and enums necessary to replicate the state
// management logic found in OpenMW's `character.hpp` and `character.cpp`.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// --- Core State Enums ---

/// Represents the broad category of action the character is performing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CharacterState {
    #[default]
    None,
    SpecialIdle,
    Idle,
    IdleSwim,
    IdleSneak,
    WalkForward,
    WalkBack,
    WalkLeft,
    WalkRight,
    SwimWalkForward,
    SwimWalkBack,
    SwimWalkLeft,
    SwimWalkRight,
    RunForward,
    RunBack,
    RunLeft,
    RunRight,
    SwimRunForward,
    SwimRunBack,
    SwimRunLeft,
    SwimRunRight,
    SneakForward,
    SneakBack,
    SneakLeft,
    SneakRight,
    TurnLeft,
    TurnRight,
    SwimTurnLeft,
    SwimTurnRight,
    Death1,
    Death2,
    Death3,
    Death4,
    Death5,
    SwimDeath,
    SwimDeathKnockDown,
    SwimDeathKnockOut,
    DeathKnockDown,
    DeathKnockOut,
    Hit,
    SwimHit,
    KnockDown,
    KnockOut,
    SwimKnockDown,
    SwimKnockOut,
    Block,
}

/// Tracks the state of the upper body, primarily for combat and spellcasting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UpperBodyState {
    #[default]
    None,
    Equipping,
    Unequipping,
    WeaponEquipped,
    AttackWindUp,
    AttackRelease,
    AttackEnd,
    Casting,
}

/// Tracks the states of jumping and landing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum JumpingStateKind {
    #[default]
    None,
    Jumping,
    InAir,
    Landing,
}

// --- State Component Structs ---

/// A component holding all state related to an actor's movement and posture.
#[derive(Component, Debug, Default)]
pub struct MovementState {
    /// The current movement mode, like walking, running, or turning.
    pub state: CharacterState,
    /// The state without any logic done to correct it, just purely from input
    pub raw_state: CharacterState,
    /// The name of the animation group currently playing for this state (e.g., "runforwardhh").
    pub current_animation_group: String,
    pub speed_multiplier: f32,
    /// The smoothed velocity vector, used to prevent jerky transitions.
    pub smoothed_speed: Vec2,
    /// Tracks if the character's primary movement is backward.
    pub is_moving_backward: bool,
    /// Timers used to determine when to grant skill increases.
    pub seconds_of_swimming: f32,
    pub seconds_of_running: f32,
    pub turn_animation_timer: f32,
    pub last_state: CharacterState,
}

/// A component holding all state related to combat, weapons, and spellcasting.
#[derive(Component, Debug, Default)]
pub struct CombatState {
    /// The current action being performed by the upper body.
    pub upper_body_state: UpperBodyState,
    /// The animation group for the current weapon action (e.g., "weapononehand:slash").
    pub current_animation_group: String,
    /// The ESM::Weapon::Type enum, stored as an integer.
    pub weapon_type: i32,
    /// The current attack type: "slash", "chop", or "thrust".
    pub attack_type: String,
    /// The charge/wind-up of the current attack, from 0.0 to 1.0.
    pub attack_strength: f32,
    /// A flag to ensure an attack's "hit" logic triggers only once per swing.
    pub ready_to_hit: bool,
    /// A flag indicating if a spell can be successfully cast.
    pub can_cast_spell: bool,
}
// Speed determined by how far the movement animation moved in this frame
// ex: runforward moved 0.1 meter forward since last frame, this would be 0.1
// used for the physics movement system to determine how far we should move in 3rd person
#[derive(Component, Default)]
pub struct AnimationDisplacement(pub Vec3);

/// A component holding all state related to jumping and falling.
#[derive(Component, Debug, Default)]
pub struct JumpingState {
    /// The current phase of the jump.
    pub state: JumpingStateKind,
    /// The name of the animation group for the current jump state (e.g., "jumphh").
    pub current_animation_group: String,
    /// These animations don't change the state so we need some other way to know to play them
    pub handled_jump_animation: bool,
    pub handled_landing_animation: bool,
}

/// A component holding all state related to health, damage reactions, and death.
#[derive(Component, Debug, Default)]
pub struct VitalsState {
    /// The current reaction to being hit, e.g., staggered or knocked down.
    pub hit_state: CharacterState,
    /// The animation group for the current hit reaction.
    pub current_hit_animation: String,
    /// The current death animation being played.
    pub death_state: CharacterState,
    /// The animation group for the current death animation.
    pub current_death_animation: String,
}

/// The primary state component for any character (player or NPC) that has animations.
/// This struct holds all the sub-state components.
#[derive(Component, Debug, Default)]
pub struct ActorState {
    // --- Posture/Intent Flags ---
    // These are set by input/AI systems.
    pub is_running: bool,
    pub is_sneaking: bool,
    pub is_flying: bool,
    pub is_in_water: bool,

    // --- Sub-structs for organization ---
    pub movement: MovementState,
    pub combat: CombatState,
    pub jumping: JumpingState,
    pub vitals: VitalsState,

    // --- Top-level state ---
    pub idle_state: CharacterState,
    pub current_idle_animation: String,

    /// A queue for scripted animations that take priority over normal state-driven animations.
    //   pub animation_queue: VecDeque<AnimationQueueEntry>,

    /// Tracks how many consecutive frames the actor has been stuck.
    pub stuck_frames: u32,
    /// Used to reset the stuck_frames counter if the actor moves far enough.
    pub last_stuck_position: Vec3,

    /// A transient flag to skip animation updates for one frame.
    pub skip_anim: bool,
}
/// A component that stores the calculated root motion displacement for a single frame.
/// This is calculated by one system and consumed by the physics movement system.
#[derive(Component, Debug, Default)]
pub struct RootMotionState {
    /// The name of the animation group that was playing last frame.
    pub last_animation_group: String,
    /// The elapsed time of the animation from the previous frame.
    pub previous_time: f32,
}
