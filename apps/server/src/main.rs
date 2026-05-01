use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    time::common_conditions::on_timer,
};
use bevy_renet::netcode::{
    NetcodeServerPlugin, NetcodeServerTransport, ServerAuthentication, ServerConfig,
};
use bevy_renet::{
    RenetServer, RenetServerEvent, RenetServerPlugin,
    renet::{ClientId, ServerEvent},
};
use common::EntityId;
use networking::{
    ClientChannel, NetworkedEntityDataSnapshot, PROTOCOL_ID, ServerChannel, ServerMessages,
    ServerRelayMessage, Velocity, connection_config,
};
use networking::{
    ClientMessage, EntityIdToEntityMap, NetworkEventWithId, NetworkedEntityData,
    ServerAcknowledgment,
};
use serializer::SerializableTransform;
use std::{
    collections::HashMap,
    f32::consts::PI,
    time::{Duration, Instant},
};
use std::{net::UdpSocket, time::SystemTime};

// How long to remember a processed action ID to prevent duplicates
const PROCESSED_ACTION_TTL: Duration = Duration::from_secs(30);

#[derive(Resource, Default, Debug)]
pub struct ProcessedActionCache(
    // Inner HashMap: Key is action_id (u64), Value is the Instant it was processed
    pub HashMap<ClientId, HashMap<u64, Instant>>,
);

#[derive(Resource)]
struct PlayerTransforms(HashMap<EntityId, Transform>);

#[derive(Resource)]
struct ClientIdToEntityId(HashMap<ClientId, EntityId>);

fn add_netcode_network(app: &mut App) {
    app.add_plugins(NetcodeServerPlugin);

    let server = RenetServer::new(connection_config());
    let public_addr = "0.0.0.0:5000".parse().unwrap();
    let socket = UdpSocket::bind(public_addr).unwrap();
    let current_time: std::time::Duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let server_config = ServerConfig {
        current_time,
        max_clients: 64,
        protocol_id: PROTOCOL_ID,
        public_addresses: vec![public_addr],
        authentication: ServerAuthentication::Unsecure,
    };

    let transport = NetcodeServerTransport::new(server_config, socket).unwrap();
    app.insert_resource(server);
    app.insert_resource(transport);
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugins((
        RenetServerPlugin,
        FrameTimeDiagnosticsPlugin {
            max_history_length: 20,
            smoothing_factor: 1.0,
        },
        LogDiagnosticsPlugin::default(),
    ));
    app.insert_resource(ProcessedActionCache::default());
    app.insert_resource(EntityIdToEntityMap::default());
    app.insert_resource(PlayerTransforms(HashMap::new()));
    app.insert_resource(ClientIdToEntityId(HashMap::new()));

    add_netcode_network(&mut app);
    app.add_systems(Startup, (setup_level, setup_simple_camera));
    app.add_systems(
        FixedUpdate,
        (
            server_update_system,
            server_broadcast_entity_data,
            server_handle_client_critical_actions_system,
        ),
    );
    app.add_observer(on_server_event);
    app.add_systems(
        Update,
        server_prune_action_cache_system.run_if(on_timer(Duration::from_secs(10))),
    );
    app.run();
}
/// set up a simple 3D scene
pub fn setup_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // plane
    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(40., 1., 40.)))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
        Transform::from_xyz(0.0, -1.0, 0.0),
    ));
    // light
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform {
            translation: Vec3::new(0.0, 2.0, 0.0),
            rotation: Quat::from_rotation_x(-PI / 4.),
            ..default()
        },
    ));
}
#[allow(clippy::too_many_arguments)]
fn on_server_event(
    server_event: On<RenetServerEvent>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lobby: ResMut<EntityIdToEntityMap>,
    mut server: ResMut<RenetServer>,
    mut player_transforms: ResMut<PlayerTransforms>,
    mut client_id_to_entity_id: ResMut<ClientIdToEntityId>,
) {
    let event = &server_event.0;
    match event {
        ServerEvent::ClientConnected { client_id } => {
            let new_player_entity_id = generate_id();

            // Tell the client its new id
            // Note this is just the one we use internally, renet is still using client_id
            let message = bincode::serialize(&ServerMessages::Welcome {
                entity_id: new_player_entity_id,
            })
            .unwrap();
            client_id_to_entity_id
                .0
                .insert(*client_id, new_player_entity_id);
            server.send_message(*client_id, ServerChannel::ServerMessages, message);

            // Spawn other players for this new client
            for (entity_id, transform) in player_transforms.0.iter() {
                let translation: [f32; 3] = transform.translation.into();
                let message = bincode::serialize(&ServerMessages::PlayerCreate {
                    entity_id: *entity_id,
                    translation,
                })
                .unwrap();
                server.send_message(*client_id, ServerChannel::ServerMessages, message);
            }
            println!("Player {} connected.", client_id);

            // Spawn new player for visualization on the server
            let transform = Transform::from_xyz(
                (fastrand::f32() - 0.5) * 40.,
                0.51,
                (fastrand::f32() - 0.5) * 40.,
            );
            let player_entity = commands
                .spawn((
                    Mesh3d(meshes.add(Mesh::from(Cuboid::default()))),
                    MeshMaterial3d(materials.add(Color::srgb(0.8, 0.7, 0.6))),
                    transform,
                ))
                .insert(Velocity::default())
                .id();

            // Add the player to the entity id -> entity map
            lobby.players.insert(new_player_entity_id, player_entity);

            // Tell everyone this player joined
            let translation: [f32; 3] = transform.translation.into();
            let message = bincode::serialize(&ServerMessages::PlayerCreate {
                entity_id: new_player_entity_id,
                translation,
            })
            .unwrap();
            server.broadcast_message(ServerChannel::ServerMessages, message);
        }
        ServerEvent::ClientDisconnected { client_id, reason } => {
            if let Some(entity_id) = client_id_to_entity_id.0.get(&client_id) {
                println!("Player {} disconnected: {}", entity_id, reason);
                if let Some(player_entity) = lobby.players.remove(entity_id) {
                    commands.entity(player_entity).despawn();
                }

                let message = bincode::serialize(&ServerMessages::PlayerRemove {
                    entity_id: *entity_id,
                })
                .unwrap();
                player_transforms.0.remove(entity_id);
                server.broadcast_message(ServerChannel::ServerMessages, message);
            }
        }
    }
}
fn server_update_system(
    mut commands: Commands,
    lobby: Res<EntityIdToEntityMap>,
    mut server: ResMut<RenetServer>,
    mut player_transforms: ResMut<PlayerTransforms>,
    client_id_to_entity_id: ResMut<ClientIdToEntityId>,
) {
    for client_id in server.clients_id() {
        while let Some(message) = server.receive_message(client_id, ClientChannel::EntityTransforms)
        {
            let input: SerializableTransform = bincode::deserialize(&message).unwrap();
            let new_transform = Transform {
                translation: input.translation,
                rotation: input.rotation,
                scale: input.scale,
            };
            if let Some(entity_id) = client_id_to_entity_id.0.get(&client_id) {
                if let Some(player_entity) = lobby.players.get(&entity_id) {
                    player_transforms.0.insert(*entity_id, new_transform);
                    commands.entity(*player_entity).insert(new_transform);
                }
            }
        }
        while let Some(message) = server.receive_message(client_id, ClientChannel::State) {
            match bincode::deserialize::<ClientMessage>(&message) {
                Ok(client_message) => {
                    match client_message {
                        ClientMessage::ActorStateUpdate(state) => {
                            // The client has told us its new state.
                            if let Some(entity_id) = client_id_to_entity_id.0.get(&client_id) {
                                // Update the server's representation of the player.
                                if let Some(player_entity) = lobby.players.get(&entity_id) {
                                    commands.entity(*player_entity).insert(state.clone());
                                }

                                // Create the relay message to send to other clients.
                                let relay_message = ServerMessages::StateUpdate {
                                    entity_id: *entity_id,
                                    state,
                                };
                                let serialized_relay_message =
                                    bincode::serialize(&relay_message).unwrap();
                                // Broadcast the message to all clients except the one who sent it.
                                server.broadcast_message_except(
                                    *entity_id,
                                    ServerChannel::StateUpdates,
                                    serialized_relay_message,
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    println!(
                        "ERROR: Failed to deserialize ClientMessage from {}: {}",
                        client_id, e
                    );
                }
            }
        }
    }
}

fn server_broadcast_entity_data(
    mut server: ResMut<RenetServer>,
    player_transforms: Res<PlayerTransforms>,
) {
    let mut networked_entities = NetworkedEntityDataSnapshot::default();
    for (entity_id, player_transform) in player_transforms.0.iter() {
        networked_entities.entity_data_map.insert(
            *entity_id,
            NetworkedEntityData {
                transform: (&player_transform.clone()).into(),
            },
        );
    }

    let sync_message = bincode::serialize(&networked_entities).unwrap();
    server.broadcast_message(ServerChannel::NetworkedEntities, sync_message);
}

pub fn setup_simple_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-20.5, 30.0, 20.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn server_handle_client_critical_actions_system(
    mut server: ResMut<RenetServer>,
    mut cache: ResMut<ProcessedActionCache>,
    client_id_to_entity_id: Res<ClientIdToEntityId>,
) {
    for client_id_u64 in server.clients_id() {
        while let Some(message_bytes) =
            server.receive_message(client_id_u64, ClientChannel::CriticalActions)
        {
            match bincode::deserialize::<NetworkEventWithId>(&message_bytes) {
                Ok(event) => {
                    info!("Server received from client {}: {:?}", client_id_u64, event);
                    let action_id = event.action_id;

                    // Get or create the map for this specific client
                    let client_specific_cache = cache.0.entry(client_id_u64).or_default();

                    // --- 0. Check if this action_id from this client was already processed ---
                    if let Some(_processed_timestamp) = client_specific_cache.get(&action_id) {
                        info!(
                            "Duplicate action (ID: {}) from client {} detected. Resending ACK.",
                            action_id, client_id_u64
                        );

                        // Action was already processed. Do NOT re-apply game effects.
                        // DO resend the acknowledgment because the client likely missed the first one.
                        let ack_message = ServerAcknowledgment::ActionConfirmed {
                            original_action_id: action_id,
                            server_validation_passed: true, // Assuming it passed previously
                            reason_if_failed: None,         // Or retrieve original reason if stored
                        };
                        if let Ok(payload) = bincode::serialize(&ack_message) {
                            server.send_message(
                                client_id_u64,
                                ServerChannel::CriticalAcknowledgments, // Your ACK channel
                                payload,
                            );
                        }
                        continue; // Stop further processing for this duplicate action
                    }

                    // --- 1. Basic Server-Side Validation (Example) ---
                    let server_validation_passed = true;
                    let reason_if_failed: Option<String> = None;
                    // Example: if action_is_implausible() { server_validation_passed = false; reason_if_failed = Some("Implausible action".to_string()); }
                    // ----------------------------------------------

                    // --- 2. Send Acknowledgment back to Originating Client ---
                    let ack_message = ServerAcknowledgment::ActionConfirmed {
                        original_action_id: event.action_id,
                        server_validation_passed,
                        reason_if_failed,
                    };
                    match bincode::serialize(&ack_message) {
                        Ok(payload) => server.send_message(
                            client_id_u64,
                            ServerChannel::CriticalAcknowledgments,
                            payload,
                        ),
                        Err(e) => error!("Failed to serialize ServerAcknowledgment: {}", e),
                    }

                    // --- 3. If Validated, Relay to Other Clients ---
                    if server_validation_passed {
                        let Some(entity_id) = client_id_to_entity_id.0.get(&client_id_u64) else {
                            continue;
                        };

                        let relay_message = ServerRelayMessage::RelayClientAction {
                            performing_entity_id: *entity_id,
                            action: event.clone(), // Send the validated action
                        };
                        match bincode::serialize(&relay_message) {
                            Ok(payload) => {
                                // Send to all *other* clients

                                for other_client_id_u64 in server.clients_id() {
                                    if other_client_id_u64 != client_id_u64 {
                                        server.send_message(
                                            other_client_id_u64,
                                            ServerChannel::CriticalRelay,
                                            payload.clone(),
                                        );
                                    }
                                }
                                info!(
                                    "Server relayed action {:?} from client {}",
                                    event, client_id_u64
                                );
                            }
                            Err(e) => error!("Failed to serialize ServerRelayMessage: {}", e),
                        }
                    }
                }
                Err(e) => error!(
                    "Server failed to deserialize ClientCriticalAction from {}: {}",
                    client_id_u64, e
                ),
            }
        }
    }
}
pub fn server_prune_action_cache_system(mut cache: ResMut<ProcessedActionCache>) {
    let current_time = Instant::now();
    let mut empty_client_caches = Vec::new();

    for (client_id, client_actions) in cache.0.iter_mut() {
        // Remove entries from this client's HashMap that are older than PROCESSED_ACTION_TTL
        client_actions.retain(|_action_id, processed_at_timestamp| {
            current_time.duration_since(*processed_at_timestamp) < PROCESSED_ACTION_TTL
        });

        // If, after pruning, this client has no cached actions, mark the client's entry for removal
        if client_actions.is_empty() {
            empty_client_caches.push(*client_id);
        }
    }

    // Remove entire client entries if their action caches became empty
    for client_id_to_remove in empty_client_caches {
        cache.0.remove(&client_id_to_remove);
        debug!(
            "Removed empty action cache for client {}",
            client_id_to_remove
        );
    }
}
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn generate_id() -> EntityId {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}
