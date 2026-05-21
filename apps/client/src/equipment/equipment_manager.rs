use crate::events::PlayerReadyWeapon;
use bevy::prelude::*;
use common::mechanics::{CharacterController, UpperBodyState};

pub fn on_player_ready_weapon(
    event: On<PlayerReadyWeapon>,
    mut actor_q: Query<&mut CharacterController>,
) {
    if let Ok(mut actor_state) = actor_q.get_mut(event.player_entity) {
        match actor_state.combat.upper_body {
            // If a weapon is already out, begin the process of unequipping it.
            UpperBodyState::WeaponEquipped => {
                actor_state.combat.upper_body = UpperBodyState::Unequipping;
                info!("State -> Unequipping");
            }
            // If hands are free (or we just finished unequipping), begin equipping.
            UpperBodyState::None => {
                actor_state.combat.upper_body = UpperBodyState::Equipping;
                info!("State -> Equipping");
            }
            // If we are in the middle of equipping or unequipping, do nothing.
            _ => {}
        }
    }
}
