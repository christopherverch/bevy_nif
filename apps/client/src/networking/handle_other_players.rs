use crate::defs::{LocalPlayer, NetworkedEntity};
use bevy::prelude::*;
use bevy_renet::RenetClient;
use common::mechanics::CharacterController;
use networking::{ServerChannel, ServerMessages};

use super::{InterpolationSpeeds, NetworkIdToEntityMap};
pub fn interpolate_players(
    mut state_query: Query<
        (Entity, &mut NetworkedEntity, &mut Transform),
        Without<super::LocalPlayer>,
    >,
    time: Res<Time>,
    interpolation_speeds: ResMut<InterpolationSpeeds>,
) {
    for (entity, player_state, mut transform) in state_query.iter_mut() {
        if let Some(interpolation_speed) = interpolation_speeds.0.get(&entity) {
            /*println!(
                            "nudging with {}, {}, {}",
                            &player_state.transform.translation,
                            *interpolation_speed,
                            time.delta_secs()
                        );
            */
            transform.translation.smooth_nudge(
                &player_state.serializable_transform.translation,
                *interpolation_speed,
                time.delta_secs(),
            );
            transform.rotation.smooth_nudge(
                &player_state.serializable_transform.rotation,
                *interpolation_speed,
                time.delta_secs(),
            );
        }
    }
}
pub fn apply_received_animation_state(
    mut client: ResMut<RenetClient>,
    lobby: Res<NetworkIdToEntityMap>,
    // We query for the full CharacterController on entities that are NOT the LocalPlayer
    mut player_query: Query<&mut CharacterController, Without<LocalPlayer>>,
) {
    while let Some(message) = client.receive_message(ServerChannel::StateUpdates) {
        // We attempt to deserialize into a ServerMessage. If it's something else, we ignore it here.
        if let Ok(server_message) = bincode::deserialize::<ServerMessages>(&message) {
            match server_message {
                // The server is telling us about another player's animation state
                ServerMessages::StateUpdate { network_id, state } => {
                    if let Some(client_server_entity) = lobby.players.get(&network_id) {
                        let client_entity = client_server_entity;

                        // Get the CharacterController component for that entity from our query
                        if let Ok(mut actor_state) = player_query.get_mut(*client_entity) {
                            actor_state.movement.state = state.character_state;
                            actor_state.combat.upper_body = state.upper_body_state;
                            actor_state.combat.weapon_type = state.weapon_type;
                            actor_state.jumping.state = state.jumping_state;
                        }
                    }
                }

                _ => {}
            }
        }
    }
}
