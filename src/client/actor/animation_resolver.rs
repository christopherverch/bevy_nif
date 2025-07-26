use bevy::prelude::*;
use tes3mp_rust::actor_defs::{ActorState, CharacterState, JumpingStateKind, UpperBodyState};

// This system runs after input is processed but before animations are applied.
// It will be responsible for overriding movement state with higher-priority
// states like jumping, getting hit, or using a weapon.
pub fn resolve_final_animation_state_system(mut query: Query<&mut ActorState>) {
    for mut actor_state in query.iter_mut() {
        // --- The Priority Cascade ---

        // 1. Vitals State (already correct)
        if actor_state.vitals.hit_state != CharacterState::None {
            actor_state.movement.state = CharacterState::None;
            actor_state.jumping.state = JumpingStateKind::None;
            continue;
        }
        // NEW: 2. Jumping State has the next highest priority.
        // This mirrors OpenMW's `refreshJumpAnims` running before `refreshMovementAnims`.
        else if matches!(
            actor_state.jumping.state,
            JumpingStateKind::Jumping | JumpingStateKind::InAir
        ) {
            actor_state.movement.state = CharacterState::None;
            continue;
        } else if matches!(
            actor_state.combat.upper_body_state,
            UpperBodyState::Equipping | UpperBodyState::Unequipping | UpperBodyState::AttackWindUp
        ) {
            // A combat action is in progress. The key here is that we *do not*
            // clear the movement state. We allow it to pass through. The animator
            // will be responsible for applying the combat animation to the upper
            // body and the movement animation to the lower body.
            // By simply continuing, we prevent the idle state from being chosen.
            continue;
        }
        // If no higher-priority state is active, the ground movement state
        // determined by the controller is allowed to pass through.
    }
}
