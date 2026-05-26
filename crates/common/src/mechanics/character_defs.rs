use std::collections::VecDeque;

use bevy_ecs::{component::Component, entity::Entity};
use bevy_math::{Vec2, Vec3};
use esm_parser_lib::types::enums::WeaponType;
use serde::{Deserialize, Serialize};

/// Attached to an entity to manage all movement, combat, and animation states.
#[derive(Component, Debug, Default)]
pub struct CharacterController {
    pub movement: MovementState,
    pub combat: CombatState,
    pub jumping: JumpingState,
    pub vitals: VitalsState,
    pub animation: AnimationEngine,
    pub targeting: ActorTargeting,
}
impl CharacterController {
    pub fn is_running(&self) -> bool {
        return self.movement.state == CharacterState::RunForward
            || self.movement.state == CharacterState::RunBack
            || self.movement.state == CharacterState::RunLeft
            || self.movement.state == CharacterState::RunRight
            || self.movement.state == CharacterState::SwimRunForward
            || self.movement.state == CharacterState::SwimRunBack
            || self.movement.state == CharacterState::SwimRunLeft
            || self.movement.state == CharacterState::SwimRunRight;
    }
    pub fn is_sneaking(&self) -> bool {
        return self.vitals.idle_state == CharacterState::IdleSneak
            || self.movement.state == CharacterState::SneakForward
            || self.movement.state == CharacterState::SneakBack
            || self.movement.state == CharacterState::SneakLeft
            || self.movement.state == CharacterState::SneakRight;
    }
    pub fn is_attacking_or_spell(&self) -> bool {
        self.combat.upper_body != UpperBodyState::None
            && self.combat.upper_body != UpperBodyState::WeaponEquipped
    }
    pub fn is_random_attack_animation(group: &str) -> bool {
        group == "attack1"
            || group == "swimattack1"
            || group == "attack2"
            || group == "swimattack2"
            || group == "attack3"
            || group == "swimattack3"
    }

    pub fn is_attack_preparing(&self) -> bool {
        self.combat.upper_body == UpperBodyState::AttackWindUp
    }

    pub fn is_casting_spell(&self) -> bool {
        self.combat.casting_scripted_spell || self.combat.upper_body == UpperBodyState::Casting
    }

    pub fn is_ready_to_block(&self) -> bool {
        // TODO:: mAnimation && updateCarriedLeftVisible(mWeaponType)
        true
    }

    pub fn is_knocked_down(&self) -> bool {
        self.vitals.hit_state == CharacterState::KnockDown
            || self.vitals.hit_state == CharacterState::SwimKnockDown
    }

    pub fn is_knocked_out(&self) -> bool {
        return self.vitals.hit_state == CharacterState::KnockOut
            || self.vitals.hit_state == CharacterState::SwimKnockOut;
    }

    pub fn is_turning(&self) -> bool {
        return self.movement.state == CharacterState::TurnLeft
            || self.movement.state == CharacterState::TurnRight
            || self.movement.state == CharacterState::SwimTurnLeft
            || self.movement.state == CharacterState::SwimTurnRight;
    }

    pub fn is_recovery(&self) -> bool {
        return self.vitals.hit_state == CharacterState::Hit
            || self.vitals.hit_state == CharacterState::SwimHit;
    }
    //TODO::
    pub fn is_scripted_anim_playing(&self) -> bool {
        // If the front of the anim queue is scripted, morrowind treats it as if it's
        // still playing even if it's actually done.
        if let Some(animation) = self.animation.queue.front() {
            return animation.scripted;
        }
        false
    }
    pub fn clear_animation_queue(&mut self) {
        //TODO::
    }
    pub fn update_anim_queue(&mut self) {

        //TODO::
    }
    pub fn is_movement_animation_controlled(
        &self,
        player_movement_ignores_animation: bool,
        is_player: bool,
    ) -> bool {
        if self.vitals.hit_state != CharacterState::None
            || self.vitals.death_state != CharacterState::None
        {
            return true;
        }
        if player_movement_ignores_animation && is_player {
            return false;
        }
        if self.jumping.in_jump {
            return false;
        }
        let mut movement_animation_controlled = self.vitals.idle_state != CharacterState::None;
        if self.movement.state != CharacterState::None {
            movement_animation_controlled = self.movement.animation_has_movement;
        }
        return movement_animation_controlled;
    }
}
#[derive(Debug, Default)]
pub struct MovementState {
    pub state: CharacterState,
    pub current_group: String,
    pub applied_turn_anim_speed: f32,
    pub curr_move_speed: f32,
    pub adjust_anim_speed: bool,
    pub animation_has_movement: bool,
    pub turn_animation_threshold: f32,
    pub is_moving_backward: bool,
    pub smoothed_speed: Vec2,
    pub seconds_of_swimming: f32,
    pub seconds_of_running: f32,
}

#[derive(Debug, Default)]
pub struct CombatState {
    pub upper_body: UpperBodyState,
    pub current_group: String,
    pub reset_idle_on_attack_end: bool,
    pub weapon_type: WeaponType,
    pub current_weapon: String,
    pub attack_strength: f32,
    pub ready_to_hit: bool,
    pub attack_success: bool,
    pub attack_type: String,
    pub can_cast: bool,
    pub casting_scripted_spell: bool,
}

#[derive(Debug, Default)]
pub struct JumpingState {
    pub state: JumpingStateKind,
    pub current_group: String,
    pub in_jump: bool,
    pub handled_landing_animation: bool,
    pub handled_jumping_animation: bool,
}

#[derive(Debug, Default)]
pub struct VitalsState {
    pub idle_state: CharacterState,
    pub current_idle_group: String,
    pub death_state: CharacterState,
    pub current_death_group: String,
    pub float_to_surface: bool,
    pub hit_state: CharacterState,
    pub current_hit_group: String,
}

#[derive(Debug, Default)]
pub struct Animation {
    pub scripted: bool,
}
#[derive(Debug, Default)]
pub struct AnimationEngine {
    //TODO:: actually store an animation index or something?
    pub queue: VecDeque<Animation>,
    pub use_lua_animations: bool,
    pub skip_anim: bool,
}

#[derive(Debug, Default)]
pub struct ActorTargeting {
    pub weapon: Option<Entity>,
    pub attack_victim: Option<Entity>,
    pub attack_hit_pos: Vec3,
    pub head_track_target: Option<Entity>,
}
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, PartialOrd)]
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
