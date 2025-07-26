use bevy::prelude::*;
use tes3mp_rust::{
    actor_defs::{ActorState, UpperBodyState},
    events::PlayerReadyWeapon,
};

pub fn on_player_ready_weapon(
    trigger: Trigger<PlayerReadyWeapon>,
    mut actor_q: Query<&mut ActorState>,
) {
    if let Ok(mut actor_state) = actor_q.get_mut(trigger.player_entity) {
        match actor_state.combat.upper_body_state {
            // If a weapon is already out, begin the process of unequipping it.
            UpperBodyState::WeaponEquipped => {
                actor_state.combat.upper_body_state = UpperBodyState::Unequipping;
                info!("State -> Unequipping");
            }
            // If hands are free (or we just finished unequipping), begin equipping.
            UpperBodyState::None => {
                actor_state.combat.upper_body_state = UpperBodyState::Equipping;
                info!("State -> Equipping");
            }
            // If we are in the middle of equipping or unequipping, do nothing.
            _ => {}
        }
    }
}
