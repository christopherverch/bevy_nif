use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    time::common_conditions::on_timer,
};
use bevy_egui::EguiPlugin;
use bevy_renet::{
    RenetServerPlugin,
    renet::{ClientId, RenetServer, ServerEvent},
};
use cdda_bevy::{
    ClientMessage, NetworkEventWithId, ServerAcknowledgment, serializer::PlayerStateNetwork,
};
use cdda_bevy::{
    ServerRelayMessage,
    networking::{
        ClientChannel, NetworkedEntities, Player, PlayerCommand, PlayerInput2, Projectile,
        ServerChannel, ServerMessages, Velocity, setup_level, spawn_fireball,
    },
};
use std::{
    collections::HashMap,
    f32::consts::PI,
    time::{Duration, Instant},
};

#[derive(Debug, Default, Resource)]
pub struct ServerLobby {
    pub players: HashMap<ClientId, Entity>,
}

// How long to remember a processed action ID to prevent duplicates
const PROCESSED_ACTION_TTL: Duration = Duration::from_secs(30);

#[derive(Resource, Default, Debug)]
pub struct ProcessedActionCache(
    // Outer HashMap: Key is ClientId (u64 from Renet)
    // Inner HashMap: Key is action_id (u64), Value is the Instant it was processed
    pub HashMap<u64, HashMap<u64, Instant>>,
);
const PLAYER_MOVE_SPEED: f32 = 5.0;

#[derive(Debug, Component)]
struct Bot {
    auto_cast: Timer,
}

#[derive(Debug, Resource)]
struct BotId(u64);

#[derive(Resource)]
struct PlayerStateNetworkHashMap(HashMap<Entity, PlayerStateNetwork>);
fn add_netcode_network(app: &mut App) {
    use bevy_renet::netcode::{
        NetcodeServerPlugin, NetcodeServerTransport, ServerAuthentication, ServerConfig,
    };
    use cdda_bevy::networking::{PROTOCOL_ID, connection_config};
    use std::{net::UdpSocket, time::SystemTime};

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

    app.add_plugins(RenetServerPlugin);
    app.add_plugins(FrameTimeDiagnosticsPlugin {
        max_history_length: 20,
        smoothing_factor: 1.0,
    });
    app.add_plugins(LogDiagnosticsPlugin::default());
    app.add_plugins(EguiPlugin {
        enable_multipass_for_primary_context: true,
    });
    app.insert_resource(ProcessedActionCache::default());
    app.insert_resource(ServerLobby::default());
    app.insert_resource(BotId(0));
    app.insert_resource(PlayerStateNetworkHashMap(HashMap::new()));

    add_netcode_network(&mut app);
    app.add_systems(
        FixedUpdate,
        (
            server_update_system,
            server_network_sync,
            move_players_system,
            update_projectiles_system,
            spawn_bot,
            bot_autocast,
            server_handle_client_critical_actions_system,
        ),
    );
    app.add_systems(
        Update,
        server_prune_action_cache_system.run_if(on_timer(Duration::from_secs(10))),
    );

    app.add_systems(FixedUpdate, apply_velocity_system);
    app.add_systems(PostUpdate, projectile_on_removal_system);
    app.add_systems(Startup, (setup_level, setup_simple_camera));
    app.run();
}

#[allow(clippy::too_many_arguments)]
fn server_update_system(
    mut server_events: EventReader<ServerEvent>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lobby: ResMut<ServerLobby>,
    mut server: ResMut<RenetServer>,
    mut player_states: ResMut<PlayerStateNetworkHashMap>,
    players: Query<(Entity, &Player, &Transform)>,
) {
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                println!("Player {} connected.", client_id);

                // Initialize other players for this new client
                for (entity, player, transform) in players.iter() {
                    let translation: [f32; 3] = transform.translation.into();
                    let message = bincode::serialize(&ServerMessages::PlayerCreate {
                        id: player.id,
                        entity,
                        translation,
                    })
                    .unwrap();
                    server.send_message(*client_id, ServerChannel::ServerMessages, message);
                }

                // Spawn new player
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
                    .insert(PlayerInput2::default())
                    .insert(Velocity::default())
                    .insert(Player { id: *client_id })
                    .id();

                lobby.players.insert(*client_id, player_entity);

                let translation: [f32; 3] = transform.translation.into();
                let message = bincode::serialize(&ServerMessages::PlayerCreate {
                    id: *client_id,
                    entity: player_entity,
                    translation,
                })
                .unwrap();
                server.broadcast_message(ServerChannel::ServerMessages, message);
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                println!("Player {} disconnected: {}", client_id, reason);
                if let Some(player_entity) = lobby.players.remove(client_id) {
                    commands.entity(player_entity).despawn();
                }

                let message =
                    bincode::serialize(&ServerMessages::PlayerRemove { id: *client_id }).unwrap();
                server.broadcast_message(ServerChannel::ServerMessages, message);
            }
        }
    }

    for client_id in server.clients_id() {
        while let Some(message) = server.receive_message(client_id, ClientChannel::Command) {
            let command: PlayerCommand = bincode::deserialize(&message).unwrap();
            match command {
                PlayerCommand::BasicAttack { mut cast_at } => {
                    println!(
                        "Received basic attack from client {}: {:?}",
                        client_id, cast_at
                    );

                    if let Some(player_entity) = lobby.players.get(&client_id) {
                        if let Ok((_, _, player_transform)) = players.get(*player_entity) {
                            cast_at[1] = player_transform.translation[1];

                            let direction =
                                (cast_at - player_transform.translation).normalize_or_zero();
                            let mut translation = player_transform.translation + (direction * 0.7);
                            translation[1] = 1.0;

                            let fireball_entity = spawn_fireball(
                                &mut commands,
                                &mut meshes,
                                &mut materials,
                                translation,
                                direction,
                            );
                            let message = ServerMessages::SpawnProjectile {
                                entity: fireball_entity,
                                translation: translation.into(),
                            };
                            let message = bincode::serialize(&message).unwrap();
                            server.broadcast_message(ServerChannel::ServerMessages, message);
                        }
                    }
                }
            }
        }
        while let Some(message) = server.receive_message(client_id, ClientChannel::Input) {
            let input: PlayerInput2 = bincode::deserialize(&message).unwrap();
            if let Some(player_entity) = lobby.players.get(&client_id) {
                commands.entity(*player_entity).insert(input);
            }
        }
        while let Some(message) =
            server.receive_message(client_id, ClientChannel::PlayerStateNetwork)
        {
            let input: PlayerStateNetwork = bincode::deserialize(&message).unwrap();
            let new_transform = Transform {
                translation: input.transform.translation,
                rotation: input.transform.rotation,
                scale: input.transform.scale,
            };

            if let Some(player_entity) = lobby.players.get(&client_id) {
                player_states.0.insert(*player_entity, input);
                commands.entity(*player_entity).insert(new_transform);
            }
        }
        while let Some(message) = server.receive_message(client_id, ClientChannel::State) {
            // Deserialize the message into our ClientMessage enum
            match bincode::deserialize::<ClientMessage>(&message) {
                Ok(client_message) => {
                    match client_message {
                        ClientMessage::StateUpdate(state) => {
                            // The client has authoritatively told us its new state.
                            // 1. Update the server's representation of the player.
                            if let Some(player_entity) = lobby.players.get(&client_id) {
                                // We insert the received state as a component on the server-side entity.
                                // This is useful for server-side logic and for the relay system.
                                commands.entity(*player_entity).insert(state.clone());
                            }

                            // 2. Create the relay message to send to other clients.
                            let relay_message = ServerMessages::StateUpdate {
                                player_id: client_id,
                                state,
                            };
                            let serialized_relay_message =
                                bincode::serialize(&relay_message).unwrap();

                            // 3. Broadcast the message to all clients *except* the one who sent it.
                            server.broadcast_message_except(
                                client_id,
                                ServerChannel::StateUpdates,
                                serialized_relay_message,
                            );
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

fn update_projectiles_system(
    mut commands: Commands,
    mut projectiles: Query<(Entity, &mut Projectile)>,
    time: Res<Time>,
) {
    for (entity, mut projectile) in projectiles.iter_mut() {
        projectile.duration.tick(time.delta());
        if projectile.duration.finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[allow(clippy::type_complexity)]
fn server_network_sync(
    mut server: ResMut<RenetServer>,
    query: Query<Entity, Or<(With<Player>, With<Projectile>)>>,
    player_states: Res<PlayerStateNetworkHashMap>,
) {
    let mut networked_entities = NetworkedEntities::default();
    for entity in query.iter() {
        if let Some(player_state) = player_states.0.get(&entity) {
            networked_entities
                .entities_and_states
                .push((entity, player_state.clone()));
        }
    }

    let sync_message = bincode::serialize(&networked_entities).unwrap();
    server.broadcast_message(ServerChannel::NetworkedEntities, sync_message);
}

fn move_players_system(mut query: Query<(&mut Velocity, &PlayerInput2)>) {
    for (mut velocity, input) in query.iter_mut() {
        let x = (input.right as i8 - input.left as i8) as f32;
        let y = (input.down as i8 - input.up as i8) as f32;
        let direction = Vec2::new(x, y).normalize_or_zero();
        velocity.0.x = direction.x * PLAYER_MOVE_SPEED;
        velocity.0.z = direction.y * PLAYER_MOVE_SPEED;
    }
}

fn apply_velocity_system(mut query: Query<(&Velocity, &mut Transform)>, time: Res<Time>) {
    for (velocity, mut transform) in query.iter_mut() {
        transform.translation += velocity.0 * time.delta_secs();
    }
}

pub fn setup_simple_camera(mut commands: Commands) {
    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-20.5, 30.0, 20.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn projectile_on_removal_system(
    mut server: ResMut<RenetServer>,
    mut removed_projectiles: RemovedComponents<Projectile>,
) {
    for entity in removed_projectiles.read() {
        let message = ServerMessages::DespawnProjectile { entity };
        let message = bincode::serialize(&message).unwrap();

        server.broadcast_message(ServerChannel::ServerMessages, message);
    }
}

fn spawn_bot(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lobby: ResMut<ServerLobby>,
    mut server: ResMut<RenetServer>,
    mut bot_id: ResMut<BotId>,
    mut commands: Commands,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        let client_id: ClientId = bot_id.0;
        bot_id.0 += 1;
        // Spawn new player
        let transform = Transform::from_xyz(
            (fastrand::f32() - 0.5) * 40.,
            0.51,
            (fastrand::f32() - 0.5) * 40.,
        );
        let player_entity = commands
            .spawn((
                Mesh3d(meshes.add(Mesh::from(Capsule3d::default()))),
                MeshMaterial3d(materials.add(Color::srgb(0.8, 0.7, 0.6))),
                transform,
            ))
            .insert(Player { id: client_id })
            .insert(Bot {
                auto_cast: Timer::from_seconds(3.0, TimerMode::Repeating),
            })
            .id();

        lobby.players.insert(client_id, player_entity);

        let translation: [f32; 3] = transform.translation.into();
        let message = bincode::serialize(&ServerMessages::PlayerCreate {
            id: client_id,
            entity: player_entity,
            translation,
        })
        .unwrap();
        server.broadcast_message(ServerChannel::ServerMessages, message);
    }
}

fn bot_autocast(
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut server: ResMut<RenetServer>,
    mut bots: Query<(&Transform, &mut Bot), With<Player>>,
    mut commands: Commands,
) {
    for (transform, mut bot) in &mut bots {
        bot.auto_cast.tick(time.delta());
        if !bot.auto_cast.just_finished() {
            continue;
        }

        for i in 0..8 {
            let direction = Vec2::from_angle(PI / 4. * i as f32);
            let direction = Vec3::new(direction.x, 0., direction.y).normalize();
            let translation: Vec3 = transform.translation + direction;

            let fireball_entity = spawn_fireball(
                &mut commands,
                &mut meshes,
                &mut materials,
                translation,
                direction,
            );
            let message = ServerMessages::SpawnProjectile {
                entity: fireball_entity,
                translation: translation.into(),
            };
            let message = bincode::serialize(&message).unwrap();
            server.broadcast_message(ServerChannel::ServerMessages, message);
        }
    }
}

pub fn server_handle_client_critical_actions_system(
    mut server: ResMut<RenetServer>,
    mut cache: ResMut<ProcessedActionCache>,
    // Potentially ResMut<GameWorldState> or specific queries for validation
) {
    for client_id_u64 in server.clients_id() {
        // Renet uses u64 for client_id

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
                        let relay_message = ServerRelayMessage::RelayClientAction {
                            performing_client_id: client_id_u64,
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
