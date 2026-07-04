use std::{net::UdpSocket, time::SystemTime};

use crate::defs::{IdToEntityMap, NeedsSetup};
use bevy::prelude::*;
use bevy_renet::{
    RenetClient,
    netcode::{ClientAuthentication, NetcodeClientTransport},
};
use networking::{PROTOCOL_ID, connection_config};

use super::EntityIdToEntityMap;
use common::{EntityId, ServerIP, state::GameState};

pub fn connect_to_server(
    mut commands: Commands,
    ip: Res<ServerIP>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    game_state.set(GameState::Offline);

    // create temp var so we don't change it until we're sure connection is OK
    let client_id_tmp = current_time.as_millis() as u64;

    let (server_addr, socket) = match (ip.0.parse(), UdpSocket::bind("0.0.0.0:0")) {
        (Ok(addr), Ok(sock)) => (addr, sock),
        (Err(e), _) => {
            eprintln!("Failed to parse server address: {}", e);
            return; // Exit the function on error
        }
        (_, Err(e)) => {
            eprintln!("Failed to bind socket: {}", e);
            return; // Exit the function on error
        }
    };
    let client = RenetClient::new(connection_config());
    commands.insert_resource(client);

    let authentication = ClientAuthentication::Unsecure {
        client_id: client_id_tmp,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };

    match NetcodeClientTransport::new(current_time, authentication, socket) {
        Ok(transport) => {
            commands.insert_resource(transport);
        }
        Err(e) => {
            eprintln!("Failed to create transport: {}", e);
            return; // Exit if transport creation fails
        }
    }
    game_state.set(GameState::Connected);
}
// Player creation logic
pub fn create_player(
    entity_id: EntityId,
    translation: [f32; 3],
    lobby: &mut EntityIdToEntityMap,
    id_to_entity_map: &mut IdToEntityMap,
    commands: &mut Commands,
) {
    dbg!(entity_id);
    dbg!(&id_to_entity_map.0);
    println!("Player {} connected. {}", entity_id, translation[1]);
    let player_transform = Transform::from_xyz(1.0, 2.0, 1.0);
    let player_root = commands
        .spawn((NeedsSetup {
            entity_id,
            player_transform,
        },))
        .id();

    let player_info = player_root;
    lobby.players.insert(entity_id, player_info);

    if id_to_entity_map.0.get(&entity_id).is_none() {
        // this should always succeed
        id_to_entity_map.0.insert(entity_id, player_root);
    } else {
        error!("tried to insert a player with an existing id!");
    }
}

// Player removal logic
pub fn remove_player(
    entity_id: EntityId,
    lobby: &mut EntityIdToEntityMap,
    id_to_entity_map: &mut IdToEntityMap,
    commands: &mut Commands,
) {
    println!("Player {} disconnected.", entity_id);
    if let Some(client_entity) = lobby.players.remove(&entity_id) {
        commands.entity(client_entity).despawn();
        id_to_entity_map.0.remove(&entity_id);
    }
}
