//! This example compares Forward, Forward + Prepass, and Deferred rendering.
use actor_funcs::apply_hit;
use bevy::{
    pbr::{DefaultOpaqueRendererMethod, DirectionalLightShadowMap},
    prelude::*,
    scene::SceneInstanceReady,
};
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_kira_audio::{AudioPlugin, SpatialAudioPlugin};
use bevy_nif::BevyNifPlugin;
use bevy_rapier3d::prelude::RapierPhysicsPlugin;
use bevy_rapier3d::prelude::*;
use bevy_renet::RenetClientPlugin;
use cdda_bevy::client::defs::*;
use cdda_bevy::client::modular_characters::*;
use cdda_bevy::client::third_person_camera::*;
use cdda_bevy::client::ui::*;
use cdda_bevy::networking::PlayerCommand;
use handle_nif_events::handle_nif_trigger;
use sound::generate_sound;
use std::f32::consts::*;
use std::time::Instant;
use terrain_material::TerrainBlendMaterial;
mod actor_funcs;
mod handle_nif_events;
mod initial_esm_parse;
mod landscape_loader;
mod player_input;
mod send_data;
mod setup_funcs;
mod terrain_material;
mod test_esm2;
use initial_esm_parse::*;
use landscape_loader::*;
use player_input::*;
use send_data::*;
use setup_funcs::*;
mod actor_animation;
mod sound;
use actor_animation::{handle_animation_request, test_animations};
#[derive(Debug)]
struct ClientServerEntity {
    client_entity: Entity,
    server_entity: Entity,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Connected;
//
/*#[derive(Component)]
struct HasWeapon {
    weapon_entity: Option<Entity>,
}*/
//we separate player and playerroot so we can have
//separate entities as children and modify them independently

fn main() {
    /*let bsa = bsa::BSAFile::new("Morrowind.bsa").unwrap();
    for file in bsa.get_list().iter() {
        println!("{}", file.name);
    }*/
    let mut app = App::new();
    app.insert_resource(DefaultOpaqueRendererMethod::deferred())
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .insert_resource(TimestepMode::Fixed {
            dt: 1.0 / 64.0,
            substeps: 1,
        })
        .add_plugins((
            DefaultPlugins,
            ThirdPersonCameraPlugin,
            BevyNifPlugin,
            UiPlugin,
            RapierPhysicsPlugin::<NoUserData>::default()
                .in_fixed_schedule()
                .with_length_unit(1.0),
            // RapierDebugRenderPlugin::default(),
            RenetClientPlugin,
            EguiPlugin {
                enable_multipass_for_primary_context: true,
            },
            WorldInspectorPlugin::default(),
            AudioPlugin,
            SpatialAudioPlugin,
        ))
        .add_plugins(MaterialPlugin::<TerrainBlendMaterial>::default())
        .insert_resource(Pause(true));
    add_netcode_network(&mut app);
    app.edit_schedule(FixedUpdate, |schedule| {
        schedule.configure_sets(
            (
                PhysicsSet::SyncBackend,
                PhysicsSet::StepSimulation,
                PhysicsSet::Writeback,
            )
                .chain(),
        );
    });
    app.add_event::<PlayerCommand>()
        .insert_resource(PlayerInput::default())
        .insert_resource(PlayerState::default())
        .insert_resource(LocalPlayer::default())
        .insert_resource(Animations::default())
        .insert_resource(IdToEntityMap::default())
        .insert_resource(SceneEntitiesByName::default())
        /*   .init_asset::<MorrowindDecodedWav>()
        .init_asset_loader::<MorrowindWavLoader>()
        .add_audio_source::<MorrowindDecodedWav>()*/
        .init_state::<GameState>()
        .init_state::<SpawnSceneState>()
        .add_systems(Startup, setup)
        .add_systems(Startup, populate_esm_resources)
        .add_systems(Update, setup_player)
        .add_systems(Update, add_nif_animation_events)
        .add_event::<SceneInstanceReady>()
        .add_systems(Update, keyboard_input)
        .add_observer(generate_sound)
        .add_observer(assemble_parts)
        .add_observer(handle_animation_request)
        .add_observer(handle_nif_trigger)
        .add_observer(trigger_attack)
        .add_observer(apply_hit)
        .add_observer(knockdown_finished)
        .add_observer(insert_nif_animator_links)
        .add_systems(Update, test_animations)
        .add_systems(Update, load_landscape)
        .add_systems(Update, setup_landscape_images)
        .add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
        .add_systems(OnEnter(GameState::Connecting), despawn_main_menu_ui)
        .add_systems(OnEnter(GameState::Connecting), connect_to_server)
        .add_systems(OnEnter(GameState::Offline), spawn_player_offline)
        .add_systems(FixedUpdate, player_movement)
        .add_systems(Update, (switch_mode, animate_light_direction))
        .run();
}
fn spawn_player_offline(mut commands: Commands) {
    info!("spawning player offline");
    commands.spawn(NeedsSetup {
        id: 0,
        player_transform: Transform::from_xyz(0.0, 0.0, 0.0),
    });

    //let player_transform_server =
    //  Transform::from_xyz(translation[0], translation[1], translation[2]);
}

fn animate_light_direction(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<DirectionalLight>>,
    pause: Res<Pause>,
    mut frames_passed: Local<u32>,
    mut last_instant: Local<Option<Instant>>,
) {
    *frames_passed += 1;
    if *frames_passed == 60 {
        //let framerate = Instant::now() - *last_instant.get_or_insert(Instant::now());
        // println!("framerate: {}", 60000.0 / framerate.as_millis() as f32);
        *last_instant = Some(Instant::now());
        *frames_passed = 0;
    }
    if pause.0 {
        return;
    }
    for mut transform in &mut query {
        transform.rotate_y(time.delta_secs() * PI / 5.0);
    }
}
