use bevy::prelude::*;
use bevy_renet::renet::RenetClient;
use cdda_bevy::{
    client::{
        actor_defs::ActorState,
        defs::{LocalPlayer, NetworkedEntity},
    },
    networking::{ServerChannel, ServerMessages},
};

use super::{EntityIdToEntityMap, InterpolationSpeeds};
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
    lobby: Res<EntityIdToEntityMap>, // Use your existing ClientLobby
    // We query for the full ActorState on entities that are NOT the LocalPlayer
    mut player_query: Query<&mut ActorState, Without<LocalPlayer>>,
) {
    while let Some(message) = client.receive_message(ServerChannel::StateUpdates) {
        // We attempt to deserialize into a ServerMessage. If it's something else, we ignore it here.
        if let Ok(server_message) = bincode::deserialize::<ServerMessages>(&message) {
            match server_message {
                // The server is telling us about another player's animation state
                ServerMessages::StateUpdate { entity_id, state } => {
                    if let Some(client_server_entity) = lobby.players.get(&entity_id) {
                        let client_entity = client_server_entity;

                        // Get the ActorState component for that entity from our query
                        if let Ok(mut actor_state) = player_query.get_mut(*client_entity) {
                            actor_state.is_running = state.is_running;
                            actor_state.is_sneaking = state.is_sneaking;
                            actor_state.is_in_water = state.is_in_water;
                            actor_state.movement.state = state.character_state;
                            actor_state.combat.upper_body_state = state.upper_body_state;
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
