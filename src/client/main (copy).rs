//! This example compares Forward, Forward + Prepass, and Deferred rendering.
mod player_input;
mod third_person_camera;
use bevy::{
    core_pipeline::{
        fxaa::Fxaa,
        prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass},
    },
    image::ImageLoaderSettings,
    math::ops,
    pbr::{
        CascadeShadowConfigBuilder, DefaultOpaqueRendererMethod, DirectionalLightShadowMap,
        NotShadowCaster, NotShadowReceiver, OpaqueRendererMethod,
    },
    prelude::*,
};
use bevy::{ecs::world, log::LogPlugin};
use bevy_rapier3d::prelude::*;
use bevy_renet::{
    RenetClientPlugin, client_connected,
    renet::{ClientId, RenetClient},
};
use bincode;
use cdda_bevy::PacketType;
use cdda_bevy::PlayerInfo;
use cdda_bevy::networking::{
    ClientChannel, ClientPlugin, NetworkEvent, NetworkedEntities, PlayerCommand, PlayerInput2,
    ServerChannel, ServerMessages, SocketAddrResource, Transport, UdpSocketResource,
    connection_config, setup_level,
};
use std::collections::HashMap;

use cdda_bevy::packet_type_from_u8;
use player_input::*;
use std::f32::consts::*;
use std::io::Cursor; // Import Cursor for easy byte slice reading
use std::net::{SocketAddr, UdpSocket};
use third_person_camera::*;
use uuid::Uuid;
//
#[derive(Component)]
struct ControlledPlayer;

#[derive(Default, Resource)]
struct NetworkMapping(HashMap<Entity, Entity>);

#[derive(Debug)]
struct PlayerInfo2 {
    client_entity: Entity,
    server_entity: Entity,
}

#[derive(Debug, Default, Resource)]
struct ClientLobby {
    players: HashMap<ClientId, PlayerInfo2>,
}

#[derive(Debug, Resource)]
struct CurrentClientId(u64);

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Connected;
//
#[derive(Event, Debug, Clone, PartialEq)] // Add the necessary traits
struct PlayerPositionUpdateEvent(PlayerInfo);
#[derive(Event)] // Add the necessary traits
struct EntitySpawnEvent {
    player_info: PlayerInfo,
    packet_type: PacketType,
}
#[derive(Component)]
struct Player {}
#[derive(Resource, Default)]
struct Players {
    // Use a HashMap to store player information, keyed by Uuid
    connected_players: std::collections::HashMap<Uuid, Entity>,
}
#[derive(Resource)]
pub struct PacketTimer(Timer);
#[derive(Component)]
struct Speed(f32);
#[derive(Component)]
struct NeedsSetup {
    id: u64,
    player_transform: Transform,
}

fn main() {
    /*let remote_addr: SocketAddr = "127.0.0.1:4567".parse().expect("could not parse addr");
    let socket = UdpSocket::bind("[::]:0").expect("could not bind socket");
    let packet_rate = 1.0 / 30.0;
    socket
        .connect(remote_addr)
        .expect("could not connect to server");
    socket
        .set_nonblocking(true)
        .expect("could not set socket to be nonblocking");*/
    let mut app = App::new();
    app.insert_resource(DefaultOpaqueRendererMethod::deferred())
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        /*.insert_resource(SocketAddrResource::new(remote_addr))
        .insert_resource(UdpSocketResource::new(socket))
        .insert_resource(PacketTimer(Timer::from_seconds(
            packet_rate,
            TimerMode::Repeating,
        )))*/
        .insert_resource(Players::default()) // Initialize the Players resource *BEFORE* adding systems that use it
        .init_resource::<PlayerInput>()
        .add_plugins((
            DefaultPlugins,
            ThirdPersonCameraPlugin,
            RapierPhysicsPlugin::<NoUserData>::default(),
            RenetClientPlugin,
        ))
        .insert_resource(Pause(true))
        .add_event::<PlayerPositionUpdateEvent>()
        .add_event::<EntitySpawnEvent>();
    add_netcode_network(&mut app);
    app.add_event::<PlayerCommand>();

    app.insert_resource(ClientLobby::default());
    app.insert_resource(PlayerInput::default());
    app.insert_resource(NetworkMapping::default());
    app.insert_resource(PlayerInput2::default());
    app.add_systems(Startup, setup);
    app.add_systems(
        Update,
        (
            client_send_input,
            client_send_player_commands,
            client_sync_players,
        )
            .in_set(Connected),
    );
    app.add_systems(Update, keyboard_input);
    app.add_systems(Update, player_movement);
    app.add_systems(Update, setup_player)
        .add_systems(Update, (animate_light_direction, switch_mode))
        .configure_sets(PostUpdate, CameraSyncSet.after(PhysicsSet::StepSimulation)) // DO THIS!
        .run();
}
fn add_netcode_network(app: &mut App) {
    use bevy_renet::netcode::{
        ClientAuthentication, NetcodeClientPlugin, NetcodeClientTransport, NetcodeTransportError,
    };
    use cdda_bevy::networking::renet::PROTOCOL_ID;
    use std::{net::UdpSocket, time::SystemTime};

    app.add_plugins(NetcodeClientPlugin);

    app.configure_sets(Update, Connected.run_if(client_connected));

    let client = RenetClient::new(connection_config());

    let server_addr = "127.0.0.1:5000".parse().unwrap();
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let client_id = current_time.as_millis() as u64;
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };

    let transport = NetcodeClientTransport::new(current_time, authentication, socket).unwrap();

    app.insert_resource(client);
    app.insert_resource(transport);
    app.insert_resource(CurrentClientId(client_id));

    // If any error is found we just panic
    #[allow(clippy::never_loop)]
    fn panic_on_error_system(mut renet_error: EventReader<NetcodeTransportError>) {
        for e in renet_error.read() {
            panic!("{}", e);
        }
    }

    app.add_systems(Update, panic_on_error_system);
}
fn player_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut player_input: ResMut<PlayerInput2>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    target_query: Query<&Transform, With<ThirdPersonCameraTarget>>,
    mut player_commands: EventWriter<PlayerCommand>,
) {
    player_input.left =
        keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft);
    player_input.right =
        keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight);
    player_input.up =
        keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp);
    player_input.down =
        keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown);

    if mouse_button_input.just_pressed(MouseButton::Left) {
        let target_transform = target_query.single();
        player_commands.send(PlayerCommand::BasicAttack {
            cast_at: target_transform.translation,
        });
    }
}

fn client_sync_players(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut client: ResMut<RenetClient>,
    asset_server: Res<AssetServer>,
    client_id: Res<CurrentClientId>,
    mut lobby: ResMut<ClientLobby>,
    mut network_mapping: ResMut<NetworkMapping>,
) { /*
    let client_id = client_id.0;
    while let Some(message) = client.receive_message(ServerChannel::ServerMessages) {
    let server_message = bincode::deserialize(&message).unwrap();
    match server_message {
    ServerMessages::PlayerCreate {
    id,
    translation,
    entity,
    } => {
    println!("Player {} connected.", id);

    let player_transform =
    Transform::from_xyz(translation[0], translation[1], translation[2]);
    if client_id == id {
    commands
    .entity(player_entity)
    .insert(ThirdPersonCameraTarget);
    }

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
    if let Some(entity) = network_mapping.0.get(&networked_entities.entities[i]) {
    let translation = networked_entities.translations[i].into();
    let transform = Transform {
    translation,
    ..Default::default()
    };
    }
    }
    }*/
}
fn client_send_input(player_input: Res<PlayerInput2>, mut client: ResMut<RenetClient>) {
    let input_message = bincode::serialize(&*player_input).unwrap();

    client.send_message(ClientChannel::Input, input_message);
}

fn client_send_player_commands(
    mut player_commands: EventReader<PlayerCommand>,
    mut client: ResMut<RenetClient>,
) {
    for command in player_commands.read() {
        let command_message = bincode::serialize(command).unwrap();
        client.send_message(ClientChannel::Command, command_message);
    }
}
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            // Deferred both supports both hdr: true and hdr: false
            hdr: true,
            ..default()
        },
        ThirdPersonCamera::default(),
        Transform::from_xyz(0.7, 0.7, 1.0).looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
        // MSAA needs to be off for Deferred rendering
        Msaa::Off,
        DistanceFog {
            color: Color::srgb_u8(43, 44, 47),
            falloff: FogFalloff::Linear {
                start: 1.0,
                end: 8.0,
            },
            ..default()
        },
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
            specular_map: asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
            intensity: 2000.0,
            ..default()
        },
        DepthPrepass,
        MotionVectorPrepass,
        DeferredPrepass,
        Fxaa::default(),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 15_000.,
            shadows_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            maximum_distance: 10.0,
            ..default()
        }
        .build(),
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, 0.0, -FRAC_PI_4)),
    ));

    // FlightHelmet
    let helmet_scene = asset_server
        .load(GltfAssetLabel::Scene(0).from_asset("models/FlightHelmet/FlightHelmet.gltf"));

    commands.spawn(SceneRoot(helmet_scene.clone()));
    commands.spawn((
        SceneRoot(helmet_scene),
        Transform::from_xyz(-4.0, 0.0, -3.0),
    ));

    let mut forward_mat: StandardMaterial = Color::srgb(0.1, 0.2, 0.1).into();
    forward_mat.opaque_render_method = OpaqueRendererMethod::Forward;
    let forward_mat_h = materials.add(forward_mat);

    // Plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(50.0, 50.0))),
        MeshMaterial3d(forward_mat_h.clone()),
        Collider::cuboid(50.0 / 2.0, 0.0 / 2.0, 50.0 / 2.0),
    ));

    let cube_h = meshes.add(Cuboid::new(0.1, 0.1, 0.1));
    let sphere_h = meshes.add(Sphere::new(0.125).mesh().uv(32, 18));

    // Cubes
    commands.spawn((
        Mesh3d(cube_h.clone()),
        MeshMaterial3d(forward_mat_h.clone()),
        Transform::from_xyz(-0.3, 0.5, -0.2),
    ));
    commands.spawn((
        Mesh3d(cube_h),
        MeshMaterial3d(forward_mat_h),
        Transform::from_xyz(0.2, 0.5, 0.2),
    ));

    let sphere_color = Color::srgb(10.0, 4.0, 1.0);
    let sphere_pos = Transform::from_xyz(0.4, 0.5, -0.8);
    // Emissive sphere
    let mut unlit_mat: StandardMaterial = sphere_color.into();
    unlit_mat.unlit = true;
    commands.spawn((
        Mesh3d(sphere_h.clone()),
        MeshMaterial3d(materials.add(unlit_mat)),
        sphere_pos,
        NotShadowCaster,
    ));
    // Light
    commands.spawn((
        PointLight {
            intensity: 800.0,
            radius: 0.125,
            shadows_enabled: true,
            color: sphere_color,
            ..default()
        },
        sphere_pos,
    ));

    // Spheres
    for i in 0..6 {
        let j = i % 3;
        let s_val = if i < 3 { 0.0 } else { 0.2 };
        let material = if j == 0 {
            materials.add(StandardMaterial {
                base_color: Color::srgb(s_val, s_val, 1.0),
                perceptual_roughness: 0.089,
                metallic: 0.0,
                ..default()
            })
        } else if j == 1 {
            materials.add(StandardMaterial {
                base_color: Color::srgb(s_val, 1.0, s_val),
                perceptual_roughness: 0.089,
                metallic: 0.0,
                ..default()
            })
        } else {
            materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, s_val, s_val),
                perceptual_roughness: 0.089,
                metallic: 0.0,
                ..default()
            })
        };
        commands.spawn((
            Mesh3d(sphere_h.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(
                j as f32 * 0.25 + if i < 3 { -0.15 } else { 0.15 } - 0.4,
                0.125,
                -j as f32 * 0.25 + if i < 3 { -0.15 } else { 0.15 } + 0.4,
            ),
        ));
    }

    // sky
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("888888").unwrap().into(),
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1_000_000.0)),
        NotShadowCaster,
        NotShadowReceiver,
    ));

    // Example instructions
    commands.spawn((
        Text::default(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));
    let player_entity = commands
        .spawn((
            Player {},
            NeedsSetup {
                id: 1,
                player_transform: Transform::from_xyz(1.0, 2.0, 1.0),
            },
            ThirdPersonCameraTarget,
        ))
        .id();
}

#[derive(Resource)]
struct Pause(bool);

fn animate_light_direction(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<DirectionalLight>>,
    pause: Res<Pause>,
) {
    if pause.0 {
        return;
    }
    for mut transform in &mut query {
        transform.rotate_y(time.delta_secs() * PI / 5.0);
    }
}
fn spawn_entity(
    mut spawn_events: EventReader<EntitySpawnEvent>,
    mut commands: Commands,
    mut players: ResMut<Players>,
    materials: ResMut<Assets<StandardMaterial>>,
    meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    if let Some(event) = spawn_events.read().next() {
        let player_info = &event.player_info;
        let packet_type = &event.packet_type;

        let player_transform = Transform::from_xyz(
            player_info.current_position.x,
            player_info.current_position.y,
            player_info.current_position.z,
        );
        let player_entity = commands.spawn(Player {}).id();
        match packet_type {
            PacketType::FirstConnected => {
                commands
                    .entity(player_entity)
                    .insert(ThirdPersonCameraTarget);
            }
            _ => {}
        }
        println!("Created player: {:?}", player_info.id); // Optional: Print the new player
        println!("Created player: {:?}", player_info.current_position); // Optional: Print the new player
        // Add the new player to the Players resource
    }
}
fn setup_player(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    mut query: Query<(Entity, &NeedsSetup), With<Player>>,
) {
    for (entity, setup_info) in &mut query {
        let name = "test".to_string();
        // The normal map. Note that to generate it in the GIMP image editor, you should
        // open the depth map, and do Filters → Generic → Normal Map
        // You should enable the "flip X" checkbox.
        let normal_handle = asset_server.load_with_settings(
            "textures/parallax_example/cube_normal.png",
            // The normal map texture is in linear color space. Lighting won't look correct
            // if `is_srgb` is `true`, which is the default.
            |settings: &mut ImageLoaderSettings| settings.is_srgb = false,
        );

        let mut cube = Mesh::from(Cuboid::new(0.15, 0.15, 0.15));

        // NOTE: for normal maps and depth maps to work, the mesh
        // needs tangents generated.
        cube.generate_tangents().unwrap();

        let parallax_material = materials.add(StandardMaterial {
            perceptual_roughness: 0.4,
            base_color_texture: Some(asset_server.load("textures/parallax_example/cube_color.png")),
            normal_map_texture: Some(normal_handle),
            // The depth map is a grayscale texture where black is the highest level and
            // white the lowest.
            depth_map: Some(asset_server.load("textures/parallax_example/cube_depth.png")),
            parallax_depth_scale: 0.09,
            parallax_mapping_method: ParallaxMappingMethod::Relief { max_steps: 4 },
            max_parallax_layer_count: ops::exp2(5.0f32),
            ..default()
        });
        let player_info = PlayerInfo {
            id: setup_info.id,
            name,
            last_position: setup_info.player_transform.translation,
            current_position: setup_info.player_transform.translation,
            last_update_time: 0.01,
            current_update_time: 0.0,
        };
        commands.entity(entity).insert((
            Mesh3d(meshes.add(cube)),
            MeshMaterial3d(parallax_material),
            setup_info.player_transform,
            Collider::cuboid(0.25, 0.5, 0.25),
            RigidBody::Dynamic,
            Damping {
                linear_damping: 5.0,
                ..default()
            },
            ExternalImpulse::default(),
            player_info,
            Speed(4.0),
        ));
    }
}
fn player_movement(
    player_input: Res<PlayerInput>,
    time: Res<Time>,
    mut player_query: Query<
        (
            &mut PlayerInfo,
            &mut ExternalImpulse,
            &mut Transform,
            &Speed,
        ),
        (With<Player>, With<ThirdPersonCameraTarget>),
    >,
    camera_query: Query<&Transform, With<Camera3d>>,
) {
    for (mut player_info, mut ext_impulse, mut player_transform, player_speed) in
        player_query.iter_mut()
    {
        player_info.current_position = player_transform.translation;
        if let Ok(camera_transform) = camera_query.get_single() {
            // Get camera's forward and right vectors (normalized)
            let forward = camera_transform.forward().normalize();
            let right = camera_transform.right().normalize();

            // Project input onto the ground plane (remove y component)
            let forward_flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
            let right_flat = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();

            // Calculate movement vector relative to camera
            let mut direction = (forward_flat * player_input.move_direction.y
                + right_flat * player_input.move_direction.x)
                * 5.0
                * time.delta_secs();
            direction.y = 0.0;
            let movement = direction.normalize_or_zero() * player_speed.0 * time.delta_secs();
            ext_impulse.impulse += movement;

            println!("player input: {}", player_input.move_direction.y);
            player_transform.look_to(direction, Vec3::Y);
            if player_input.jump {
                println!("hit jump: {}", time.elapsed_secs());
                ext_impulse.impulse += Vec3::new(0.0, 1., 0.0);
            }
        }
    }
}

#[derive(Resource, Default)]
enum DefaultRenderMode {
    #[default]
    Deferred,
    Forward,
    ForwardPrepass,
}

#[allow(clippy::too_many_arguments)]
fn switch_mode(
    mut text: Single<&mut Text>,
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut default_opaque_renderer_method: ResMut<DefaultOpaqueRendererMethod>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cameras: Query<Entity, With<Camera>>,
    mut pause: ResMut<Pause>,
    mut hide_ui: Local<bool>,
    mut mode: Local<DefaultRenderMode>,
) {
    text.clear();

    if keys.just_pressed(KeyCode::Space) {
        pause.0 = !pause.0;
    }

    if keys.just_pressed(KeyCode::Digit1) {
        *mode = DefaultRenderMode::Deferred;
        default_opaque_renderer_method.set_to_deferred();
        println!("DefaultOpaqueRendererMethod: Deferred");
        for _ in materials.iter_mut() {}
        for camera in &cameras {
            commands.entity(camera).remove::<NormalPrepass>();
            commands.entity(camera).insert(DepthPrepass);
            commands.entity(camera).insert(MotionVectorPrepass);
            commands.entity(camera).insert(DeferredPrepass);
        }
    }
    if keys.just_pressed(KeyCode::Digit2) {
        *mode = DefaultRenderMode::Forward;
        default_opaque_renderer_method.set_to_forward();
        println!("DefaultOpaqueRendererMethod: Forward");
        for _ in materials.iter_mut() {}
        for camera in &cameras {
            commands.entity(camera).remove::<NormalPrepass>();
            commands.entity(camera).remove::<DepthPrepass>();
            commands.entity(camera).remove::<MotionVectorPrepass>();
            commands.entity(camera).remove::<DeferredPrepass>();
        }
    }
    if keys.just_pressed(KeyCode::Digit3) {
        *mode = DefaultRenderMode::ForwardPrepass;
        default_opaque_renderer_method.set_to_forward();
        println!("DefaultOpaqueRendererMethod: Forward + Prepass");
        for _ in materials.iter_mut() {}
        for camera in &cameras {
            commands.entity(camera).insert(NormalPrepass);
            commands.entity(camera).insert(DepthPrepass);
            commands.entity(camera).insert(MotionVectorPrepass);
            commands.entity(camera).remove::<DeferredPrepass>();
        }
    }

    if keys.just_pressed(KeyCode::KeyH) {
        *hide_ui = !*hide_ui;
    }

    if !*hide_ui {
        text.push_str("(H) Hide UI\n");
        text.push_str("(Space) Play/Pause\n\n");
        text.push_str("Rendering Method:\n");

        text.push_str(&format!(
            "(1) {} Deferred\n",
            if let DefaultRenderMode::Deferred = *mode {
                ">"
            } else {
                ""
            }
        ));
        text.push_str(&format!(
            "(2) {} Forward\n",
            if let DefaultRenderMode::Forward = *mode {
                ">"
            } else {
                ""
            }
        ));
        text.push_str(&format!(
            "(3) {} Forward + Prepass\n",
            if let DefaultRenderMode::ForwardPrepass = *mode {
                ">"
            } else {
                ""
            }
        ));
    }
}
