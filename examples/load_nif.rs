use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_nif::nif_animation::{BlendMask, NifAnimator, SkeletonMap};
use bevy_nif::*;
use bevy_third_person_camera::*;
mod setup;

const BLEND_MASK_ROOTS: [&str; 3] = [
    "Bip01 Spine1",     /* Torso */
    "Bip01 L Clavicle", /* Left arm */
    "Bip01 R Clavicle", /* Right arm */
];
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BevyNifPlugin)
        .insert_resource(AmbientLight {
            // Add ambient light resource
            color: Color::srgb(1.0, 0.8, 0.6), // Match warm tone
            brightness: 100.0,
            affects_lightmapped_meshes: true,
        })
        .add_plugins(ThirdPersonCameraPlugin)
        .add_systems(Startup, setup::setup_scene)
        .add_systems(Startup, setup::setup)
        .add_systems(Update, test_animations)
        .add_plugins(EguiPlugin {
            enable_multipass_for_primary_context: false,
        })
        .add_plugins(WorldInspectorPlugin::new())
        .run();
}

fn test_animations(
    mut animator_q: Query<(&mut AnimationPlayer, &NifAnimator, &AnimationGraphHandle)>,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    mut run_once: Local<bool>,
    skeleton_map_res: Res<SkeletonMap>,
) {
    if !*run_once {
        if animator_q.iter().len() < 1 {
            return;
        }
        for (id, skeleton) in &skeleton_map_res.skeletons {
            for (mut animation_player, nif_animator, graph_handle) in animator_q.iter_mut() {
                if nif_animator.skeleton_id != *id {
                    continue;
                }
                let mut animation_names = Vec::new();
                for (name, animation) in &nif_animator.animation_definitions {
                    animation_names.push(name);
                }
                animation_names.sort();
                for animation in animation_names {
                    println!(" animation: {}", animation);
                }
                let anim_graph = anim_graphs.get_mut(graph_handle).unwrap();
                let walk_back_animation = nif_animator
                    .animation_definitions
                    .get("RunForward2w_loop")
                    .unwrap();

                let pickprobe_animation = nif_animator
                    .animation_definitions
                    .get("WeaponTwoWide: Chop")
                    .unwrap();
                if let Some(anim_graph_node) = anim_graph.get_mut(pickprobe_animation.node_index) {
                    let mut mask: u64 = 0;
                    for bone_data in skeleton.get_all_children("Bip01 L Thigh") {
                        mask |= 1 << bone_data.id.0 as u32;
                    }
                    for bone_data in skeleton.get_all_children("Bip01 R Thigh") {
                        mask |= 1 << bone_data.id.0 as u32;
                    }
                    println!("mask: {:b}", mask);
                    println!("mask: {}", BlendMask::UPPER_BODY.bits());
                    anim_graph_node.mask = 1;
                }

                let walk_anim_graph_node =
                    anim_graph.get_mut(walk_back_animation.node_index).unwrap();
                walk_anim_graph_node.mask = 14;
                animation_player
                    .play(pickprobe_animation.node_index)
                    .seek_to(0.0)
                    .repeat();
                animation_player
                    .play(walk_back_animation.node_index)
                    .seek_to(0.0)
                    .repeat();
                *run_once = true;
            }
        }
    }
}
