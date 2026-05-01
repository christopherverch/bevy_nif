use bevy::prelude::*;
use handle_other_players::{apply_received_animation_state, interpolate_players};
use networking::EntityIdToEntityMap;
use send_data::{
    add_critical_network_event, client_handle_relayed_actions_system,
    client_handle_server_acks_system, client_send_transform, client_sync_players,
    client_trigger_attack_system, send_local_player_state_system,
};

use super::Connected;
use crate::defs::{
    ClientEntityId, LocalPlayer, NetworkEventWithIdCounter, PendingServerAcks, ServerIP,
};
use bevy_renet::client_connected;
use bevy_renet::netcode::NetcodeClientPlugin;
use std::collections::HashMap;

mod connecting_disconnecting;
mod handle_other_players;
mod send_data;
pub use connecting_disconnecting::connect_to_server;
pub struct NetworkingPlugin;

// Maps an entity's network id to the entity
#[derive(Resource, Default)]
pub struct InterpolationSpeeds(HashMap<Entity, f32>);

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(NetcodeClientPlugin)
            .configure_sets(Update, Connected.run_if(client_connected))
            .configure_sets(FixedUpdate, Connected.run_if(client_connected))
            .insert_resource(EntityIdToEntityMap::default())
            .insert_resource(InterpolationSpeeds::default())
            .insert_resource(ServerIP(("127.0.0.1").to_string())) // Default IP Address
            .insert_resource(NetworkEventWithIdCounter(0))
            .insert_resource(PendingServerAcks(HashMap::new()))
            .insert_resource(ClientEntityId(0));

        app.add_systems(
            FixedUpdate,
            (
                client_send_transform,
                send_local_player_state_system,
                apply_received_animation_state,
            )
                .in_set(Connected),
        )
        .add_systems(
            Update,
            (
                client_sync_players,
                interpolate_players.after(client_sync_players),
                client_trigger_attack_system,
                client_handle_server_acks_system,
                client_handle_relayed_actions_system,
            )
                .in_set(Connected),
        )
        .add_observer(add_critical_network_event);
    }
}
