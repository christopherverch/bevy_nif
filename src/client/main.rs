//! This example compares Forward, Forward + Prepass, and Deferred rendering.
use actor_animator::animate_actors_using_state;
use actor_funcs::apply_hit;
use bevy::{
    pbr::{DefaultOpaqueRendererMethod, DirectionalLightShadowMap},
    prelude::*,
};
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_kira_audio::{AudioPlugin, SpatialAudioPlugin};
use bevy_nif::BevyNifPlugin;
use bevy_rapier3d::prelude::RapierPhysicsPlugin;
use bevy_rapier3d::prelude::*;
use bevy_renet::RenetClientPlugin;
use camera::MWCameraPlugin;
use cdda_bevy::client::defs::*;
use cdda_bevy::client::ui::*;
use cdda_bevy::networking::PlayerCommand;
use combat::trigger_attack;
use handle_nif_events::handle_nif_trigger;
use landscape::LandscapePlugin;
use movement::{apply_root_motion_system, player_movement};
use sound::generate_sound;
use temporary_funcs::{animate_light_direction, switch_mode};
use terrain_material::TerrainBlendMaterial;
mod actor_animator;
mod actor_funcs;
mod camera;
mod combat;
mod const_defs;
mod handle_nif_events;
mod initial_esm_parse;
mod landscape;
mod movement;
mod network_systems;
mod player_input;
mod send_data;
mod setup_funcs;
mod sound;
mod temporary_funcs;
mod terrain_material;
mod utils;

use initial_esm_parse::*;
use player_input::*;
use send_data::*;
use setup_funcs::*;
#[derive(Debug)]
struct ClientServerEntity {
    client_entity: Entity,
    server_entity: Entity,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Connected;

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
            BevyNifPlugin,
            MWCameraPlugin,
            MWInputPlugin,
            LandscapePlugin,
            UiPlugin,
            RapierPhysicsPlugin::<NoUserData>::default()
                .in_fixed_schedule()
                .with_length_unit(1.0),
            RapierDebugRenderPlugin::default(),
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
        .insert_resource(IdToEntityMap::default())
        .init_state::<GameState>()
        .add_systems(Startup, setup_scene)
        .add_systems(Update, setup_player)
        .add_systems(Startup, populate_esm_resources)
        .add_systems(Update, add_nif_animation_events)
        .add_systems(Update, keyboard_input)
        .add_observer(generate_sound)
        .add_observer(handle_nif_trigger)
        .add_observer(trigger_attack)
        .add_observer(apply_hit)
        .add_systems(
            PostUpdate,
            // Add the new system. It must run after animations are applied.
            apply_root_motion_system,
        )
        .add_observer(insert_nif_animator_links)
        .add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
        .add_systems(OnEnter(GameState::Connecting), despawn_main_menu_ui)
        .add_systems(OnEnter(GameState::Connecting), connect_to_server)
        .add_systems(OnEnter(GameState::Offline), spawn_player_offline)
        .add_systems(FixedUpdate, player_movement)
        .add_systems(Update, (switch_mode, animate_light_direction))
        .add_systems(Update, animate_actors_using_state)
        .run();
}
