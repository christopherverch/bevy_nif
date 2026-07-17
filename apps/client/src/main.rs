// External imports
use avian3d::{PhysicsPlugins, prelude::PhysicsSystems};
use bevy::{
    app::HierarchyPropagatePlugin, camera::visibility::RenderLayers,
    light::DirectionalLightShadowMap, pbr::DefaultOpaqueRendererMethod, prelude::*,
};
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_kira_audio::{AudioPlugin, SpatialAudioPlugin};
use bevy_nif::{BevyNifPlugin, nif_animation::SkeletonMap};
use bevy_renet::RenetClientPlugin;
use common::{
    NextAvailableSkeletonId,
    state::{CharCreationState, GameState},
};
use serde::Deserialize;
use std::{env, fs};

// Internal imports
use crate::{
    animation::handle_animation_transitions,
    defs::{GameSettings, IdToEntityMap, Pause, PlayerInput, WorldFrameData, WorldSettings},
    handle_nif_events::handle_nif_event,
    mechanics::MWMechanicsPlugin,
    physics::{MWPhysicsPlugin, interpolate_visual_positions},
    setup_scene::setup_scene,
};
use camera::MWCameraPlugin;
use equipment::on_player_ready_weapon;
use gui::MWGuiPlugin;
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
mod setup;
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
        .insert_resource(WorldFrameData::default())
        .insert_resource(WorldSettings::default())
        .insert_resource(GameSettings::default())
        // Default to 1 because 0 is used by the initial player
        .insert_resource(NextAvailableSkeletonId::default())
        .init_state::<GameState>()
        .init_state::<CharCreationState>()
        .add_systems(Startup, setup_scene)
        .add_systems(Update, setup_player)
        .add_systems(Startup, populate_esm_resources)
        .add_observer(handle_nif_event)
        .add_observer(on_player_ready_weapon)
        .add_observer(insert_nif_animator_links)
        .add_systems(OnEnter(GameState::Offline), spawn_player_offline)
        .add_systems(Update, (switch_mode, animate_light_direction))
        .add_systems(Update, handle_animation_transitions)
        //.add_systems(Update, animate_actors_using_state)
        .add_systems(Update, interpolate_visual_positions)
        //.add_systems(Update, wireframe)
        .add_systems(PostUpdate, clear_player_rotation_input_system)
        .run();
}
#[allow(dead_code)]
fn wireframe(
    skeleton_map_res: Res<SkeletonMap>,
    entity_q: Query<(Option<&Children>, &GlobalTransform)>,
    mut gizmos: Gizmos,
) {
    for (_id, skeleton_entity) in &skeleton_map_res.root_skeleton_entity_map {
        if let Ok((children_opt, global_transform)) = entity_q.get(*skeleton_entity) {
            // Use the first child of the skeleton entity to draw the wireframe, the other one is
            // the dark elf skinned mesh which we don't want to draw a wireframe for
            let Some(children) = children_opt else { return };
            recursive_wireframe(
                *children.first().unwrap(),
                global_transform.translation(),
                &entity_q,
                &mut gizmos,
            );
        }
    }
}
fn recursive_wireframe(
    parent_entity: Entity,
    // Pass the parent's world position (if it exists)
    parent_global_pos: Vec3,
    entity_q: &Query<(Option<&Children>, &GlobalTransform)>,
    gizmos: &mut Gizmos,
) {
    if let Ok((children_opt, gt)) = entity_q.get(parent_entity) {
        let current_pos = gt.translation();

        // 1. Draw the line to the parent
        gizmos.line_gradient(
            parent_global_pos,
            current_pos,
            Color::srgb(1.0, 0.0, 0.0),
            Color::WHITE,
        );

        // 2. Draw the visual marker for this node
        gizmos.cube(
            Transform::from_translation(current_pos)
                .with_rotation(gt.compute_transform().rotation)
                .with_scale(Vec3::splat(0.02)),
            Color::srgb(1.0, 0.0, 0.0),
        );

        // 3. Recurse
        if let Some(children) = children_opt {
            for &child in children {
                // Pass the current entity's position as the parent position for the next call
                recursive_wireframe(child, current_pos, entity_q, gizmos);
            }
        }
    }
}
