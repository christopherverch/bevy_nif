use super::{GameState, ServerIP};
use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use cdda_bevy::serializer::{PlayerStateNetwork, SerializableTransform};
use std::time::Duration;

use super::{Connected, NeedsSetup, PlayerInfo2, PlayerRoot, PlayerState};
use bevy_renet::netcode::{
    ClientAuthentication, NetcodeClientPlugin, NetcodeClientTransport, NetcodeTransportError,
};
use bevy_renet::{
    client_connected,
    renet::{ClientId, RenetClient},
};
use cdda_bevy::networking::renet::PROTOCOL_ID;
use cdda_bevy::networking::{
    ClientChannel, NetworkedEntities, PlayerCommand, ServerChannel, ServerMessages,
    connection_config,
};
use std::collections::HashMap;
use std::{net::UdpSocket, time::SystemTime};

#[derive(Default, Resource)]
pub struct NetworkMapping(HashMap<Entity, Entity>);
#[derive(Debug, Resource)]
pub struct CurrentClientId(pub u64);

#[derive(Debug, Default, Resource)]
pub struct ClientLobby {
    players: HashMap<ClientId, PlayerInfo2>,
}

pub fn add_netcode_network(app: &mut App) {
    app.add_plugins(NetcodeClientPlugin)
        .configure_sets(Update, Connected.run_if(client_connected))
        .insert_resource(NetworkMapping::default())
        .insert_resource(ClientLobby::default());
    app.insert_resource(ServerIP(("127.0.0.1").to_string())); // Default IP Address

    app.add_systems(
        Update,
        (
            client_send_input,
            client_send_player_commands,
            client_sync_players,
            serialize_player_system.run_if(on_timer(Duration::from_secs(30))),
        )
            .in_set(Connected),
    );

    // If any error is found we just panic
    #[allow(clippy::never_loop)]
    fn panic_on_error_system(mut renet_error: EventReader<NetcodeTransportError>) {
        for e in renet_error.read() {
            panic!("{}", e);
        }
    }

    app.add_systems(Update, panic_on_error_system);
}
pub fn connect_to_server(
    // New system to handle connection (and transport creation)
    mut commands: Commands,
    ip: Res<ServerIP>,
    mut game_state: ResMut<NextState<GameState>>,
    mut client_id: ResMut<CurrentClientId>,
) {
    game_state.set(GameState::Offline);

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let client_id_tmp = current_time.as_millis() as u64; //create temp var so we don't change it
    //until we're sure connection is OK
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
    client_id.0 = client_id_tmp;
}
pub fn client_send_input(player_state: Res<PlayerState>, mut client: ResMut<RenetClient>) {
    let player_state_network = PlayerStateNetwork {
        transform: SerializableTransform::from(&player_state.transform),
    };
    let input_message = bincode::serialize(&player_state_network).unwrap();
    client.send_message(ClientChannel::PlayerStateNetwork, input_message);
}

pub fn client_send_player_commands(
    mut player_commands: EventReader<PlayerCommand>,
    mut client: ResMut<RenetClient>,
) {
    for command in player_commands.read() {
        let command_message = bincode::serialize(command).unwrap();
        client.send_message(ClientChannel::Command, command_message);
    }
}

pub fn serialize_player_system(player_state: Res<PlayerState>, mut client: ResMut<RenetClient>) {
    let player_state = PlayerStateNetwork {
        transform: SerializableTransform::from(&player_state.transform),
    };
    // Serialize it
    match bincode::serialize(&player_state) {
        Ok(serialized) => {
            println!("Serialized PlayerStateNetwork: {:?}", serialized);
            client.send_message(ClientChannel::PlayerStateNetwork, serialized);
            //send over network
        }
        Err(err) => {
            eprintln!("Serialization error: {}", err);
        }
    }
}

pub fn client_sync_players(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut client: ResMut<RenetClient>,
    client_id: Res<CurrentClientId>,
    mut lobby: ResMut<ClientLobby>,
    mut network_mapping: ResMut<NetworkMapping>,
) {
    let client_id = client_id.0;
    while let Some(message) = client.receive_message(ServerChannel::ServerMessages) {
        let server_message = bincode::deserialize(&message).unwrap();
        match server_message {
            ServerMessages::PlayerCreate {
                id,
                translation,
                entity,
            } => {
                println!("Player {} connected. {}", id, translation[1]);
                let player_transform = Transform::from_xyz(1.0, 2.0, 1.0);
                let player_entity = commands
                    .spawn((NeedsSetup {
                        id,
                        player_transform,
                    },))
                    .id();

                //let player_transform_server =
                //  Transform::from_xyz(translation[0], translation[1], translation[2]);

                let player_info = PlayerInfo2 {
                    server_entity: entity,
                    client_entity: player_entity,
                };
                lobby.players.insert(id, player_info);
                network_mapping.0.insert(entity, player_entity);
            }
            ServerMessages::PlayerRemove { id } => {
                println!("Player {} disconnected.", id);
                if let Some(PlayerInfo2 {
                    server_entity,
                    client_entity,
                }) = lobby.players.remove(&id)
                {
                    commands.entity(client_entity).despawn();
                    network_mapping.0.remove(&server_entity);
                }
            }
            ServerMessages::SpawnProjectile {
                entity,
                translation,
            } => {
                let projectile_entity = commands.spawn((
                    Mesh3d(meshes.add(Mesh::from(Sphere::new(0.1)))),
                    MeshMaterial3d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
                    Transform::from_translation(translation.into()),
                ));
                network_mapping.0.insert(entity, projectile_entity.id());
            }
            ServerMessages::DespawnProjectile { entity } => {
                if let Some(entity) = network_mapping.0.remove(&entity) {
                    commands.entity(entity).despawn();
                }
            }
        }
    }

    while let Some(message) = client.receive_message(ServerChannel::NetworkedEntities) {
        let networked_entities: NetworkedEntities = bincode::deserialize(&message).unwrap();

        for i in 0..networked_entities.entities.len() {
            if i as u64 != client_id {
                if let Some(entity) = network_mapping.0.get(&networked_entities.entities[i]) {
                    let translation = networked_entities.translations[i].into();
                    let transform = Transform {
                        translation,
                        ..Default::default()
                    };

                    let test = lobby.players.get(&client_id);
                    match test {
                        Some(value) => {
                            if entity == &value.client_entity {
                                continue;
                            }
                        }
                        None => {
                            println!("No value");
                            // Handle the None case
                        }
                    }
                    commands.entity(*entity).insert(transform);
                }
            }
        }
    }
}
