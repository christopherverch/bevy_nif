// External imports
use avian3d::{PhysicsPlugins, prelude::PhysicsSystems};
use bevy::{
    app::HierarchyPropagatePlugin, camera::visibility::RenderLayers,
    light::DirectionalLightShadowMap, pbr::DefaultOpaqueRendererMethod, prelude::*,
};
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_kira_audio::{AudioPlugin, SpatialAudioPlugin};
use bevy_nif::BevyNifPlugin;
use bevy_renet::RenetClientPlugin;
use serde::Deserialize;
use std::{env, fs};

// Internal imports
use crate::{
    defs::{
        CellLookup, GameSettings, GameState, IdToEntityMap, Pause, PlayerInput, WorldFrameData,
        WorldSettings,
    },
    gui::MWGuiPlugin,
    handle_nif_events::handle_nif_event,
    mechanics::MWMechanicsPlugin,
    physics::{MWPhysicsPlugin, interpolate_visual_positions},
    setup_scene::setup_scene,
};
use animation::actor_animator::animate_actors_using_state;
use camera::MWCameraPlugin;
use equipment::on_player_ready_weapon;
use initial_esm_parse::*;
use landscape::LandscapePlugin;
use networking::NetworkingPlugin;
use player_input::*;
use setup_funcs::*;
use temporary_funcs::{animate_light_direction, switch_mode};

mod actor;
mod animation;
mod camera;
mod const_defs;
mod defs;
mod engine;
mod equipment;
mod esm;
mod events;
mod gui;
mod handle_nif_events;
mod initial_esm_parse;
mod landscape;
mod live_cell_ref;
mod magic_effects;
mod mechanics;
mod networking;
mod physics;
mod player_input;
mod setup_funcs;
mod setup_scene;
mod stat;
mod temporary_funcs;
mod utils;
mod world;

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
    // Load config file
    let path = env::var("CARGO_MANIFEST_DIR");
    if let Ok(mut path) = path {
        path.push_str("/config.toml");
        let config_text = fs::read_to_string(&path).expect("Could not read config");
        let config: Config = toml::from_str(&config_text).expect("Invalid config format");
        dbg!(config.data_dirs);
        dbg!(config.content);
    }

    let mut app = App::new();
    app.insert_resource(DefaultOpaqueRendererMethod::deferred())
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .add_plugins((
            PhysicsPlugins::default(),
            HierarchyPropagatePlugin::<RenderLayers>::new(PostUpdate),
        ))
        .add_plugins((
            DefaultPlugins,
            EguiPlugin::default(),
            WorldInspectorPlugin::default(),
            bevy_framepace::FramepacePlugin,
            // Internal plugins
            BevyNifPlugin,
            MWInputPlugin,
            MWMechanicsPlugin,
            MWGuiPlugin,
            MWPhysicsPlugin,
            LandscapePlugin,
            RenetClientPlugin,
            AudioPlugin,
            SpatialAudioPlugin,
            NetworkingPlugin,
        ))
        .add_plugins(MWCameraPlugin)
        .insert_resource(Pause(true));
    app.edit_schedule(FixedUpdate, |schedule| {
        schedule.configure_sets(
            (
                PhysicsSystems::Prepare,
                PhysicsSystems::StepSimulation,
                PhysicsSystems::Writeback,
            )
                .chain(),
        );
    });
    app.insert_resource(PlayerInput::default())
        .insert_resource(IdToEntityMap::default())
        .insert_resource(NextAvailableSkeletonId(1))
        .insert_resource(WorldFrameData::default())
        .insert_resource(WorldSettings::default())
        .insert_resource(GameSettings::default())
        .insert_resource(CellLookup::default())
        .init_state::<GameState>()
        .add_systems(Startup, setup_scene)
        .add_systems(Update, setup_player)
        .add_systems(Startup, populate_esm_resources)
        .add_observer(handle_nif_event)
        .add_observer(on_player_ready_weapon)
        .add_observer(insert_nif_animator_links)
        .add_systems(OnEnter(GameState::Offline), spawn_player_offline)
        .add_systems(Update, (switch_mode, animate_light_direction))
        .add_systems(Update, animate_actors_using_state)
        .add_systems(Update, interpolate_visual_positions)
        .add_systems(PostUpdate, clear_player_rotation_input_system)
        .run();
}
