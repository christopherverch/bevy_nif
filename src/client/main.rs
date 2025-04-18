//! This example compares Forward, Forward + Prepass, and Deferred rendering.
mod player_input;
mod send_data;
mod third_person_camera;
use bevy::{
    color::palettes::basic::*,
    core_pipeline::{
        fxaa::Fxaa,
        prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass},
        //prepass::NormalPrepass
    },
    image::ImageLoaderSettings,
    math::ops,
    pbr::{
        CascadeShadowConfigBuilder, DefaultOpaqueRendererMethod, DirectionalLightShadowMap,
        NotShadowCaster, NotShadowReceiver, OpaqueRendererMethod,
    },
    prelude::*,
    ui::FocusPolicy,
};

use bevy_rapier3d::prelude::*;
use bevy_renet::RenetClientPlugin;
//use bsatoollib as bsa;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy_rapier3d::prelude::{RapierPhysicsPlugin, TimestepMode};
use cdda_bevy::networking::PlayerCommand;
use cdda_bevy::serializer::PlayerState;
use player_input::*;
use send_data::*;
use std::f32::consts::*;
use third_person_camera::*;

#[derive(Component)]
struct HudText;
#[derive(Clone, Copy, Debug, Eq, PartialEq, States, Default, Hash)]
pub enum GameState {
    #[default]
    MainMenu,
    Connecting,
    Offline,
    Connected,
}
#[derive(Debug)]
struct PlayerInfo2 {
    client_entity: Entity,
    server_entity: Entity,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Connected;
//
#[derive(Component)]
pub struct PlayerRoot;
#[derive(Component)]
pub struct Player;
//we separate player and playerroot so we can have
//separate entities as children and modify them independently
#[derive(Component)]
struct NeedsSetup {
    id: u64,
    player_transform: Transform,
}

#[derive(Component)]
struct IpInputField;

#[derive(Component)]
struct PlayerCamera;
#[derive(Component)]
struct ConnectButton;

#[derive(Resource)]
struct ServerIP(String);
#[derive(Component)]
struct MainMenuRoot;
#[derive(Component)]
struct Weapon;
#[derive(Component)]
struct LocalPlayer;
#[derive(Resource, Default)]
struct FocusedIpInput {
    entity: Option<Entity>, // Use Option<Entity>
}

const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::srgb(0.35, 0.75, 0.35);
fn main() {
    /*let bsa = bsa::BSAFile::new("Morrowind.bsa").unwrap();
    for file in bsa.get_list().iter() {
        println!("{}", file.name);
    }*/
    let mut app = App::new();
    app.insert_resource(DefaultOpaqueRendererMethod::deferred())
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        /*  .insert_resource(TimestepMode::Fixed {
            dt: 1.0 / 64.0,
            substeps: 1,
        })*/
        .add_plugins((
            DefaultPlugins,
            ThirdPersonCameraPlugin,
            RapierPhysicsPlugin::<NoUserData>::default()
                //   .in_fixed_schedule()
                .with_length_unit(10.0),
            RenetClientPlugin,
        ))
        .insert_resource(Pause(true));
    add_netcode_network(&mut app);
    /*  app.edit_schedule(FixedUpdate, |schedule| {
        schedule.configure_sets(
            (
                PhysicsSet::SyncBackend,
                PhysicsSet::StepSimulation,
                PhysicsSet::Writeback,
            )
                .chain(),
        );
    });*/
    app.add_event::<PlayerCommand>()
        .insert_resource(PlayerInput::default())
        .insert_resource(PlayerState::default())
        .insert_resource(FocusedIpInput::default())
        .insert_resource(CurrentClientId(0))
        .init_state::<GameState>()
        .add_systems(Startup, setup)
        .add_systems(Update, setup_player)
        .add_systems(Update, keyboard_input)
        /*       .add_systems(
            PostUpdate,
            camera_movement.after(PhysicsSet::StepSimulation),
        )*/
        .add_systems(
            Update,
            listen_keyboard_input_events.run_if(in_state(GameState::MainMenu)),
        )
        .add_systems(Update, button_system.run_if(in_state(GameState::MainMenu)))
        .add_systems(Update, attach_camera_to_player)
        .add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
        .add_systems(OnEnter(GameState::MainMenu), set_initial_focus)
        .add_systems(OnEnter(GameState::Connecting), connect_to_server)
        .add_systems(OnEnter(GameState::Connecting), despawn_main_menu_ui)
        .add_systems(OnEnter(GameState::Offline), spawn_player_offline)
        .add_systems(Update, player_movement)
        .add_systems(Update, (switch_mode, animate_light_direction))
        .configure_sets(PostUpdate, CameraSyncSet.after(PhysicsSet::StepSimulation)) // DO THIS!
        .run();
}
fn spawn_player_offline(mut commands: Commands) {
    commands.spawn(NeedsSetup {
        id: 0,
        player_transform: Transform::from_xyz(0.0, 0.0, 0.0),
    });

    //let player_transform_server =
    //  Transform::from_xyz(translation[0], translation[1], translation[2]);
}
fn attach_camera_to_player(
    mut commands: Commands,
    player_query: Query<Entity, Added<PlayerRoot>>,
    mut camera_query: Query<(Entity, &mut Transform), With<PlayerCamera>>,
) {
    if let Ok(player_entity) = player_query.get_single() {
        if let Ok((camera_entity, mut camera_transform)) = camera_query.get_single_mut() {
            camera_transform.translation = Vec3::ZERO;
            commands.entity(player_entity).add_child(camera_entity);
        }
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
        PlayerCamera,
        ThirdPersonCamera::default(),
        Transform::from_xyz(0.7, 0.7, 1.0).looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
        // MSAA needs to be off for Deferred rendering
        Msaa::Off,
        DistanceFog {
            color: Color::srgb_u8(43, 44, 47),
            falloff: FogFalloff::Linear {
                start: 5.0,
                end: 20.0,
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
    let font = asset_server.load("fonts/FiraMono-Medium.ttf");
    commands.spawn((
        Text2d::new(""),
        TextFont {
            font,
            font_size: 100.0,
            ..default()
        },
    ));
    /*commands
    .spawn(Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    })
    .with_children(|parent| {
        parent
            .spawn((
                Button,
                Node {
                    width: Val::Px(150.0),
                    height: Val::Px(65.0),
                    border: UiRect::all(Val::Px(5.0)),
                    // horizontally center child text
                    justify_content: JustifyContent::Center,
                    // vertically center child text
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor(Color::BLACK),
                BorderRadius::MAX,
                BackgroundColor(NORMAL_BUTTON),
            ))
            .with_child((
                Text::new("Button"),
                TextFont {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 33.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
    });*/
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
    let woodelf_scene =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/woodelf.glb"));
    commands.spawn((
        SceneRoot(woodelf_scene),
        Transform::from_xyz(-2.0, 0.0, -3.0),
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
    let rotation = Quat::from_rotation_y(45f32.to_radians()); // Convert degrees to radians
    let cube_h = meshes.add(Cuboid::new(0.1, 0.1, 0.1));
    let cube_h2 = meshes.add(Cuboid::new(2.0, 2.0, 2.0));
    commands.spawn((
        Mesh3d(cube_h2),
        MeshMaterial3d(forward_mat_h.clone()),
        Collider::cuboid(2.0 / 2.0, 2.0 / 2.0, 2.0 / 2.0),
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, 0.0, -FRAC_PI_4)),
    ));
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
    let font_handle = asset_server.load("fonts/FiraMono-Medium.ttf");
    commands.spawn((
        // Accepts a `String` or any type that converts into a `String`, such as `&str`
        Text::new("test"),
        TextFont {
            // This font is loaded and will be used instead of the default font.
            font: font_handle,
            font_size: 25.0,
            ..default()
        },
        // Set the justification of the Text
        TextLayout::new_with_justify(JustifyText::Left),
        // Set the style of the Node itself.
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
        HudText,
    ));
}

fn setup_main_menu(mut commands: Commands, asset_server: Res<AssetServer>) {
    println!("spawning menu");
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            MainMenuRoot,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        padding: UiRect::all(Val::Px(20.0)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(GRAY.into()), // Assuming NORMAL_BUTTON is Color::GREEN
                ))
                .with_children(|parent| {
                    // Text label
                    let font_handle = asset_server.load("fonts/FiraMono-Medium.ttf");
                    parent.spawn((
                        Text::new("Enter Server IP:"),
                        TextFont {
                            font: font_handle.clone(),
                            font_size: 30.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        TextLayout::default(), // Default justification
                    ));

                    // Input field
                    parent
                        .spawn((
                            Node {
                                width: Val::Px(200.0),
                                height: Val::Px(30.0),
                                margin: UiRect::bottom(Val::Px(10.0)),
                                padding: UiRect::all(Val::Px(5.0)),
                                ..default()
                            },
                            BackgroundColor(WHITE.into()), // Assuming NORMAL_BUTTON is Color::GREEN
                        ))
                        .with_children(|parent| {
                            parent.spawn((
                                Text::new(""),
                                TextFont {
                                    font: font_handle.clone(),
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(Color::BLACK),
                                TextLayout::default(),
                                IpInputField,
                            ));
                        })
                        .insert(Interaction::default())
                        .insert(FocusPolicy::Pass); // Make it focusable

                    // Connect button
                    parent
                        .spawn((
                            Button,
                            Node {
                                // The main button Node
                                width: Val::Px(150.0),
                                height: Val::Px(65.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(5.0)), // The Border component
                                ..default()
                            },
                            BorderColor(Color::BLACK),
                            BackgroundColor(GREEN.into()), // Assuming NORMAL_BUTTON is Color::GREEN
                            BorderRadius::MAX,
                            ConnectButton,
                            Interaction::default(),
                        ))
                        .with_children(|parent| {
                            parent.spawn((
                                Text::new("Connect"),
                                TextFont {
                                    font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                                    font_size: 33.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.9, 0.9, 0.9)),
                                TextLayout::default(),
                            ));
                        });
                });
        });
}
fn despawn_main_menu_ui(mut commands: Commands, mut query: Query<Entity, With<MainMenuRoot>>) {
    for entity in &mut query {
        commands.entity(entity).despawn_recursive();
    }
}

fn set_initial_focus(
    focused_input: Res<FocusedIpInput>,
    mut interaction_query: Query<&mut Interaction>,
) {
    if let Some(entity) = focused_input.entity {
        // Directly access the Option<Entity>
        if let Ok(mut interaction) = interaction_query.get_mut(entity) {
            *interaction = Interaction::Pressed; // Simulate a click to focus.
        }
    }
}

fn listen_keyboard_input_events(
    mut events: EventReader<KeyboardInput>,
    text_query: Single<&mut Text, With<IpInputField>>,
    focused_input: Option<Res<FocusedIpInput>>,
    mut game_state: ResMut<NextState<GameState>>,
    mut server_addr: ResMut<ServerIP>,
) {
    if focused_input.is_some() {
        let mut text = text_query.into_inner();
        for event in events.read() {
            // Only trigger changes when the key is first pressed.
            if !event.state.is_pressed() {
                continue;
            }

            match &event.logical_key {
                Key::Enter => {
                    if text.is_empty() {
                        server_addr.0 = "".to_string();
                        game_state.set(GameState::Connecting);
                        println!("ip: {}", server_addr.0);
                        return;
                        //   let old_value = mem::take(&mut **text);
                    }
                    match text.0.as_str() {
                        "" => {
                            server_addr.0 = text.0.clone();
                        }
                        "1" => {
                            server_addr.0 = "127.0.0.1:5000".to_string();
                        }
                        _ => {
                            server_addr.0 = "107.136.161.231:5000".to_string();
                        }
                    }
                    game_state.set(GameState::Connecting);
                    println!("server addr: {}", server_addr.0);
                }
                Key::Space => {
                    text.push(' ');
                }
                Key::Backspace => {
                    text.pop();
                }
                Key::Character(character) => {
                    text.push_str(character);
                }
                _ => continue,
            }
        }
    }
}

fn button_system(
    mut interaction_query: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            &Children,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    mut text_query: Query<&mut Text, Without<IpInputField>>,
    mut game_state: ResMut<NextState<GameState>>,
    mut server_addr: ResMut<ServerIP>,
    ip_input_field: Single<&mut Text, With<IpInputField>>,
) {
    for (interaction, mut color, mut border_color, children) in &mut interaction_query {
        let mut text = text_query.get_mut(children[0]).unwrap();
        match *interaction {
            Interaction::Pressed => {
                **text = "Press".to_string();
                *color = PRESSED_BUTTON.into();
                border_color.0 = RED.into();
                game_state.set(GameState::Connecting);
                server_addr.0 = ip_input_field.0.clone();
                println!("ip: {}", server_addr.0);
            }
            Interaction::Hovered => {
                **text = "Hover".to_string();
                *color = HOVERED_BUTTON.into();
                border_color.0 = Color::WHITE;
            }
            Interaction::None => {
                **text = "Button".to_string();
                *color = NORMAL_BUTTON.into();
                border_color.0 = Color::BLACK;
            }
        }
    }
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
fn setup_player(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    mut query: Query<(Entity, &NeedsSetup)>,
    client_id: Res<CurrentClientId>,
) {
    for (entity, setup_info) in &mut query {
        println!("setting up");
        commands.entity(entity).insert((
            PlayerRoot,
            Transform::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
        ));
        let player = commands.spawn(Player).id();
        commands.entity(entity).add_child(player);
        //add player as a child so we won't rotate the camera when rotating the player

        println!("client id: {} {}", client_id.0, setup_info.id);
        if client_id.0 == setup_info.id {
            commands.entity(player).insert(ThirdPersonCameraTarget);
            commands.entity(player).insert(LocalPlayer);
        }
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
        commands.entity(player).insert((
            Mesh3d(meshes.add(cube.clone())),
            MeshMaterial3d(parallax_material.clone()),
            Transform::from_xyz(0.0, 1.0, 0.0),
            Collider::capsule_y(0.5, 1.0),
            KinematicCharacterController {
                custom_mass: Some(5.0),
                up: Vec3::Y,
                offset: CharacterLength::Absolute(0.03),
                slide: true,
                autostep: Some(CharacterAutostep {
                    max_height: CharacterLength::Relative(0.3),
                    min_width: CharacterLength::Relative(0.5),
                    include_dynamic_bodies: false,
                }),
                // Don’t allow climbing slopes larger than 45 degrees.
                max_slope_climb_angle: 43.0_f32.to_radians(),
                // Automatically slide down on slopes smaller than 30 degrees.
                min_slope_slide_angle: 30.0_f32.to_radians(),
                apply_impulse_to_dynamic_bodies: true,
                snap_to_ground: Some(CharacterLength::Relative(0.2)),
                ..default()
            },
            Speed(4.0),
        ));
        commands.entity(entity).with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(cube)),
                Transform::from_xyz(-0.2, 0.0, -0.2),
                MeshMaterial3d(parallax_material),
                TransformInterpolation::default(),
                Weapon,
            ));
        });
        println!(
            "player id and transform: {}{}",
            setup_info.id, setup_info.player_transform.translation
        );
        commands.entity(entity).remove::<NeedsSetup>();
    }
}

#[derive(Resource, Default)]
enum DefaultRenderMode {
    #[default]
    Deferred,
}

#[allow(clippy::too_many_arguments)]
fn switch_mode(
    mut query: Query<&mut Text, With<HudText>>, // Changed to a regular Query with a filter
    keys: Res<ButtonInput<KeyCode>>,
    mut pause: ResMut<Pause>,
) {
    let mut text = match query.get_single_mut() {
        Ok(t) => t,
        Err(e) => {
            match e {
                bevy::ecs::query::QuerySingleError::NoEntities(_) => {
                    eprintln!("Warning: No UI text entity found with the HudText component.");
                }
                bevy::ecs::query::QuerySingleError::MultipleEntities(_) => {
                    eprintln!(
                        "Warning: Multiple UI text entities found with the HudText component. Expected only one."
                    );
                }
            }
            return; // Return early if there's an error
        }
    };
    text.clear();

    if keys.just_pressed(KeyCode::Space) {
        pause.0 = !pause.0;
    }
    /*
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
    }*/
}
