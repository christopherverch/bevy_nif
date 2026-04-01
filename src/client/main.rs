use std::{env, fs};

use actor::{
    actor_funcs::apply_hit,
    controller::{character_controller_system, resolve_final_animation_state_system},
};
use animation::actor_animator::animate_actors_using_state;
use bevy::{light::DirectionalLightShadowMap, pbr::DefaultOpaqueRendererMethod, prelude::*};
use serde::Deserialize;
use tes3mp_rust::defs::{GameState, IdToEntityMap, Pause, PlayerInput};
mod networking;
mod setup_scene;
pub mod ui;
use crate::{handle_nif_events::handle_nif_event, setup_scene::setup_scene};
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_kira_audio::{AudioPlugin, SpatialAudioPlugin};
use bevy_nif::BevyNifPlugin;
use bevy_rapier3d::prelude::RapierPhysicsPlugin;
use bevy_rapier3d::prelude::*;
use bevy_renet::RenetClientPlugin;
use camera::MWCameraPlugin;
use equipment::on_player_ready_weapon;
use landscape::LandscapePlugin;
mod animation;
mod const_defs;
mod physics;
use landscape::terrain_material::TerrainBlendMaterial;
use networking::{NetworkingPlugin, connect_to_server};
use physics::{
    CharacterMotionPlugin, calculate_animation_displacement_system,
    movement::character_movement_system, preparation::prepare_character_velocity_system,
};
use sound::generate_sound;
use temporary_funcs::{animate_light_direction, switch_mode};
pub mod actor;
pub mod camera;
mod equipment;
mod gui;
mod handle_nif_events;
mod initial_esm_parse;
mod landscape;
mod player_input;
mod setup_funcs;
mod sound;
mod temporary_funcs;
mod utils;
use initial_esm_parse::*;
use player_input::*;
use setup_funcs::*;
use ui::{UiPlugin, despawn_main_menu_ui, setup_main_menu};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Connected;
#[derive(Deserialize, Debug)]
struct Config {
    data_dirs: Vec<String>,
    content: Vec<String>,
}
#[derive(Resource)]
pub struct NextAvailableSkeletonId(pub u64);
fn main() {
    /*let bsa = bsa::BSAFile::new("Morrowind.bsa").unwrap();
    for file in bsa.get_list().iter() {
        println!("{}", file.name);
    }*/
    let path = env::var("CARGO_MANIFEST_DIR");
    if let Ok(mut path) = path {
        path.push_str("/config.toml");
        let config_text = fs::read_to_string(&path).expect("Could not read config");
        let config: Config = toml::from_str(&config_text).expect("Invalid config format");
        dbg!(config);
    }

    let mut app = App::new();
    app.insert_resource(DefaultOpaqueRendererMethod::deferred())
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .insert_resource(TimestepMode::Fixed {
            dt: 1.0,
            substeps: 60,
        })
        .add_plugins((
            DefaultPlugins,
            BevyNifPlugin,
            CharacterMotionPlugin,
            MWInputPlugin,
            LandscapePlugin,
            UiPlugin,
            RapierPhysicsPlugin::<NoUserData>::default()
                .in_fixed_schedule()
                .with_length_unit(1.0),
            RapierDebugRenderPlugin::default(),
            RenetClientPlugin,
            EguiPlugin::default(),
            WorldInspectorPlugin::default(),
            AudioPlugin,
            SpatialAudioPlugin,
            NetworkingPlugin,
        ))
        .add_plugins(MWCameraPlugin)
        .add_plugins(MaterialPlugin::<TerrainBlendMaterial>::default())
        .insert_resource(Pause(true));
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
    app.insert_resource(PlayerInput::default())
        .insert_resource(IdToEntityMap::default())
        .insert_resource(NextAvailableSkeletonId(1))
        .init_state::<GameState>()
        .add_systems(Startup, setup_scene)
        .add_systems(Update, setup_player)
        .add_systems(Startup, populate_esm_resources)
        .add_observer(generate_sound)
        .add_observer(handle_nif_event)
        .add_observer(apply_hit)
        .add_observer(on_player_ready_weapon)
        .add_observer(insert_nif_animator_links)
        .add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
        .add_systems(OnEnter(GameState::Connecting), despawn_main_menu_ui)
        .add_systems(OnEnter(GameState::Connecting), connect_to_server)
        .add_systems(OnEnter(GameState::Offline), spawn_player_offline)
        .add_systems(Update, (switch_mode, animate_light_direction))
        .add_systems(
            FixedUpdate,
            (
                // The new controller system takes the place of the old one.
                calculate_animation_displacement_system,
                prepare_character_velocity_system,
                character_movement_system,
            )
                .chain()
                .before(PhysicsSet::SyncBackend),
        )
        .add_systems(
            Update,
            (
                character_controller_system,
                resolve_final_animation_state_system,
                animate_actors_using_state,
            )
                .chain(),
        )
        .add_systems(PostUpdate, clear_player_rotation_input_system)
        .run();
}
