//use cdda_bevy::client::animation::play_animation_local;

use super::GameState;
use super::{ClientServerEntity, Connected};
use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use bevy_renet::netcode::{
    ClientAuthentication, NetcodeClientPlugin, NetcodeClientTransport, NetcodeTransportError,
};
use bevy_renet::{
    client_connected,
    renet::{ClientId, RenetClient},
};
use cdda_bevy::client::defs::{
    CurrentClientId, LocalPlayer, MeshEntityLink, NeedsSetup, NetworkedEntity, PlayerState,
    ServerIP,
};
use cdda_bevy::client::events::RequestAnimation;
use cdda_bevy::networking::renet::PROTOCOL_ID;
use cdda_bevy::networking::{
    ClientChannel, NetworkedEntities, PlayerCommand, ServerChannel, ServerMessages,
    connection_config,
};
use cdda_bevy::serializer::PlayerStateNetwork;
use std::collections::HashMap;
use std::time::Duration;
use std::time::Instant;
use std::{net::UdpSocket, time::SystemTime};

#[derive(Default, Resource)]
pub struct NetworkMapping(HashMap<Entity, Entity>);

#[derive(Debug, Default, Resource)]
pub struct ClientLobby {
    players: HashMap<ClientId, ClientServerEntity>,
}
#[derive(Resource, Default)]
pub struct InterpolationSpeeds(HashMap<Entity, f32>);
pub fn add_netcode_network(app: &mut App) {
    app.add_plugins(NetcodeClientPlugin)
        .configure_sets(Update, Connected.run_if(client_connected))
        .configure_sets(FixedUpdate, Connected.run_if(client_connected))
        .insert_resource(NetworkMapping::default())
        .insert_resource(ClientLobby::default())
        .insert_resource(InterpolationSpeeds::default())
        .insert_resource(ServerIP(("127.0.0.1").to_string())); // Default IP Address

    app.add_systems(
        FixedUpdate,
        (
            client_send_input,
            serialize_player_system.run_if(on_timer(Duration::from_secs(30))),
        )
            .in_set(Connected),
    )
    .add_systems(
        Update,
        (
            client_send_player_commands,
            client_sync_players,
            interpolate_players.after(client_sync_players),
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
pub fn client_send_input(
    local_player_state_query: Query<&PlayerState, (With<LocalPlayer>, Changed<PlayerState>)>,
    mut client: ResMut<RenetClient>,
) {
    if let Ok(local_player_state) = local_player_state_query.get_single() {
        let player_state_network = PlayerStateNetwork::from(&*local_player_state);
        let input_message = bincode::serialize(&player_state_network).unwrap();
        client.send_message(ClientChannel::PlayerStateNetwork, input_message);
    }
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

pub fn serialize_player_system(
    local_player_state: Single<&PlayerState, (With<LocalPlayer>, Changed<PlayerState>)>,
    mut client: ResMut<RenetClient>,
) {
    let player_state = PlayerStateNetwork::from(local_player_state.into_inner());
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

pub fn interpolate_players(
    mut state_query: Query<
        (Entity, &MeshEntityLink, &mut PlayerState, &mut Transform),
        Without<super::LocalPlayer>,
    >,
    mut commands: Commands,
    time: Res<Time>,
    interpolation_speeds: ResMut<InterpolationSpeeds>,
) {
    for (entity, mesh_entity, player_state, mut transform) in state_query.iter_mut() {
        if let Some(interpolation_speed) = interpolation_speeds.0.get(&entity) {
            /*println!(
                            "nudging with {}, {}, {}",
                            &player_state.transform.translation,
                            *interpolation_speed,
                            time.delta_secs()
                        );
            */
            transform.translation.smooth_nudge(
                &player_state.transform.translation,
                *interpolation_speed,
                time.delta_secs(),
            );
            transform.rotation.smooth_nudge(
                &player_state.transform.rotation,
                *interpolation_speed,
                time.delta_secs(),
            );
            if let Some(mesh_entity) = mesh_entity.0 {
                let mut mesh_transform = *transform;
                mesh_transform.translation.y -= 1.0;
                commands.entity(mesh_entity).insert(mesh_transform);
            }
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
    mut player_state_query: Query<(Entity, &NetworkedEntity, &mut PlayerState)>,
    mut interpolation_speeds: ResMut<InterpolationSpeeds>,
) {
    let client_id = client_id.0;

    // Process server messages (player creation, removal, projectiles)
    process_server_messages(
        &mut client,
        &mut lobby,
        &mut network_mapping,
        &mut commands,
        &mut meshes,
        &mut materials,
    );

    // Process networked entities and update player state
    process_networked_entities(
        &mut client,
        &mut player_state_query,
        &mut network_mapping,
        &mut interpolation_speeds,
        &mut commands,
        &mut lobby,
        client_id,
    );
}

fn process_server_messages(
    client: &mut RenetClient,
    lobby: &mut ClientLobby,
    network_mapping: &mut NetworkMapping,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    while let Some(message) = client.receive_message(ServerChannel::ServerMessages) {
        let server_message = bincode::deserialize(&message).unwrap();
        match server_message {
            ServerMessages::PlayerCreate {
                id,
                translation,
                entity,
            } => {
                create_player(id, translation, entity, lobby, network_mapping, commands);
            }
            ServerMessages::PlayerRemove { id } => {
                remove_player(id, lobby, network_mapping, commands);
            }
            ServerMessages::SpawnProjectile {
                entity,
                translation,
            } => {
                spawn_projectile(
                    entity,
                    translation,
                    network_mapping,
                    commands,
                    meshes,
                    materials,
                );
            }
            ServerMessages::DespawnProjectile { entity } => {
                despawn_projectile(entity, network_mapping, commands);
            }
        }
    }
}

fn process_networked_entities(
    client: &mut RenetClient,
    player_state_query: &mut Query<(Entity, &NetworkedEntity, &mut PlayerState)>,
    network_mapping: &mut NetworkMapping,
    interpolation_speeds: &mut ResMut<InterpolationSpeeds>,
    commands: &mut Commands,
    lobby: &mut ClientLobby,
    client_id: u64,
) {
    while let Some(message) = client.receive_message(ServerChannel::NetworkedEntities) {
        let networked_entities: NetworkedEntities = bincode::deserialize(&message).unwrap();

        for (networked_entity, player_state_network) in
            networked_entities.entities_and_states.into_iter()
        {
            if let Some(local_entity) = network_mapping.0.get(&networked_entity) {
                if !should_process_local_entity(local_entity, lobby, client_id) {
                    continue;
                }

                update_player_state(
                    local_entity,
                    player_state_network,
                    player_state_query,
                    commands,
                    interpolation_speeds,
                );
            }
        }
    }
}

// Player creation logic
fn create_player(
    id: u64,
    translation: [f32; 3],
    entity: Entity,
    lobby: &mut ClientLobby,
    network_mapping: &mut NetworkMapping,
    commands: &mut Commands,
) {
    println!("Player {} connected. {}", id, translation[1]);
    let player_transform = Transform::from_xyz(1.0, 2.0, 1.0);
    let player_root = commands
        .spawn((NeedsSetup {
            id,
            player_transform,
        },))
        .id();

    let player_info = ClientServerEntity {
        server_entity: entity,
        client_entity: player_root,
    };
    lobby.players.insert(id, player_info);
    network_mapping.0.insert(entity, player_root);
}

// Player removal logic
fn remove_player(
    id: u64,
    lobby: &mut ClientLobby,
    network_mapping: &mut NetworkMapping,
    commands: &mut Commands,
) {
    println!("Player {} disconnected.", id);
    if let Some(ClientServerEntity {
        server_entity,
        client_entity,
    }) = lobby.players.remove(&id)
    {
        commands.entity(client_entity).despawn();
        network_mapping.0.remove(&server_entity);
    }
}

// Spawn projectile logic
fn spawn_projectile(
    entity: Entity,
    translation: [f32; 3],
    network_mapping: &mut NetworkMapping,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let projectile_entity = commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Sphere::new(0.1)))),
        MeshMaterial3d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
        Transform::from_translation(translation.into()),
    ));
    network_mapping.0.insert(entity, projectile_entity.id());
}

// Despawn projectile logic
fn despawn_projectile(
    entity: Entity,
    network_mapping: &mut NetworkMapping,
    commands: &mut Commands,
) {
    if let Some(entity) = network_mapping.0.remove(&entity) {
        commands.entity(entity).despawn();
    }
}

// Check whether the local entity should be processed
fn should_process_local_entity(
    local_entity: &Entity,
    lobby: &mut ClientLobby,
    client_id: u64,
) -> bool {
    if let Some(client_server_entity) = lobby.players.get(&client_id) {
        if local_entity == &client_server_entity.client_entity {
            return false;
        }
    } else {
        eprintln!("Server is not tracking local player position!");
    }
    true
}

// Update player state and animations
fn update_player_state(
    local_entity: &Entity,
    player_state_network: PlayerStateNetwork,
    player_state_query: &mut Query<(Entity, &NetworkedEntity, &mut PlayerState)>,
    commands: &mut Commands,
    interpolation_speeds: &mut ResMut<InterpolationSpeeds>,
) {
    for (player_entity, networked_entity, mut player_state) in player_state_query.iter_mut() {
        if networked_entity.0 != *local_entity {
            continue;
        }
        //past this we should have the player_state corresponding to the networked entity

        // Update interpolation speed
        let elapsed = player_state.last_packet_time.elapsed().as_millis() as f32;
        let speed = if elapsed > 0.0 { 400.0 / elapsed } else { 10.0 };
        interpolation_speeds.0.insert(player_entity, speed);
        player_state.last_packet_time = Instant::now();

        // Trigger animations if changed

        //if the server sent us a packet that this player's animation is different, play it
        if player_state_network.requested_primary_animation
            != player_state.requested_primary_animation
        {
            dbg!(&player_state.requested_primary_animation);
            dbg!(&player_state_network.requested_primary_animation);
            if let Some(animation) = player_state_network.requested_primary_animation {
                println!(
                    "requesting prim animation for player with id: {} {:?} -> {:?}",
                    player_state.id,
                    player_state.requested_primary_animation,
                    player_state_network.requested_primary_animation,
                );
                commands.trigger(RequestAnimation {
                    entity: player_entity,
                    requested_animation: animation,
                });
            }
        }

        if player_state_network.requested_secondary_animation
            != player_state.requested_secondary_animation
        {
            if let Some(animation) = player_state_network.requested_secondary_animation {
                println!(
                    "requesting second animation for player with id: {} {:?} -> {:?}",
                    player_state.id,
                    player_state.requested_secondary_animation,
                    player_state_network.requested_secondary_animation,
                );
                commands.trigger(RequestAnimation {
                    entity: player_entity,
                    requested_animation: animation,
                });
            }
        }
        // Update the player state from the network
        player_state.update_from_network(player_state_network);
        break;
    }
}
