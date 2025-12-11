use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_nif::nif_animation::bevy_types::{ActiveAnimation, Priority};
use bevy_nif::nif_animation::{BlendMask, NifAnimator, SkeletonMap};
use bevy_nif::*;
use bevy_rapier3d::plugin::{NoUserData, RapierPhysicsPlugin};
use bevy_rapier3d::render::RapierDebugRenderPlugin;
use bevy_third_person_camera::*;
use setup::setup_nif_physics;
mod setup;

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
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugins(RapierDebugRenderPlugin::default())
        .add_plugins(ThirdPersonCameraPlugin)
        .add_systems(Startup, setup::setup_scene)
        .add_systems(Startup, setup::setup)
        .add_systems(Update, test_animations)
        .add_systems(Update, test_loop_anims)
        .add_systems(Update, setup_nif_physics)
        .add_plugins(EguiPlugin {
            enable_multipass_for_primary_context: false,
        })
        .add_plugins(WorldInspectorPlugin::new())
        .run();
}
fn test_loop_anims(
    mut animator_q: Query<(
        &mut AnimationPlayer,
        &mut NifAnimator,
        &AnimationGraphHandle,
    )>,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    skeleton_map_res: Res<SkeletonMap>,
    animation_clips: Res<Assets<AnimationClip>>,
) {
    for (id, _skeleton) in &skeleton_map_res.skeletons {
        for (mut animation_player, mut nif_animator, graph_handle) in animator_q.iter_mut() {
            if nif_animator.skeleton_id != *id {
                continue;
            }
            let Some(run_forward_anim) = nif_animator.active_animations.get("runforward2w") else {
                return;
            };
            let runforward_clip = animation_clips.get(&run_forward_anim.clip_handle).unwrap();
            let duration = runforward_clip.duration();
            println!(
                "duration: {}, elapsed: {}",
                duration,
                animation_player
                    .animation(run_forward_anim.node_index)
                    .unwrap()
                    .elapsed()
            );
            if animation_player
                .animation(run_forward_anim.node_index)
                .unwrap()
                .elapsed()
                >= duration
            {
                animation_player.stop(run_forward_anim.node_index);
                let run_forward_anim = nif_animator
                    .animation_definitions
                    .get("runforward2w_loop")
                    .unwrap();
                let anim_graph = anim_graphs.get_mut(graph_handle).unwrap();
                let run_forward_graph_node =
                    anim_graph.get_mut(run_forward_anim.node_index).unwrap();
                run_forward_graph_node.mask = BlendMask::UPPER_BODY.bits();
                animation_player.play(run_forward_anim.node_index).repeat();

                nif_animator.active_animations.remove("runforward2w");
                println!("time to loop");
            }
        }
    }
}
fn test_animations(
    mut animator_q: Query<
        (
            &mut AnimationPlayer,
            &mut NifAnimator,
            &AnimationGraphHandle,
        ),
        Added<NifAnimator>,
    >,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    skeleton_map_res: Res<SkeletonMap>,
    animation_clips: Res<Assets<AnimationClip>>,
) {
    for (id, _skeleton) in &skeleton_map_res.skeletons {
        for (mut animation_player, mut nif_animator, graph_handle) in animator_q.iter_mut() {
            if nif_animator.skeleton_id != *id {
                continue;
            }
            // --- print out all animation names ---
            let mut animation_names = Vec::new();
            for (name, animation) in &nif_animator.animation_definitions {
                animation_names.push((name, animation.clone()));
            }
            animation_names.sort_by_key(|pair| pair.0.clone());
            /*for (animation, animation_def) in animation_names {
                println!(
                    " animation: {}, {:?}",
                    animation,
                    animation_clips
                        .get(&animation_def.clip_handle)
                        .unwrap()
                        .duration()
                );
                dbg!(animation_def.min_hit_time_relative);
                dbg!(animation_def.hit_time_relative);
            }*/
            // ------------------------------------

            let anim_graph = anim_graphs.get_mut(graph_handle).unwrap();
            let run_forward_anim = nif_animator
                .animation_definitions
                .get("runforward2w")
                .unwrap();

            let chop_animation = nif_animator
                .animation_definitions
                .get("weapontwohand: chop_release")
                .unwrap();
            let chop_anim_graph_node = anim_graph.get_mut(chop_animation.node_index).unwrap();
            chop_anim_graph_node.mask = BlendMask::LOWER_BODY.bits();
            println!("chop mask: {}", BlendMask::LOWER_BODY.bits());
            let run_forward_graph_node = anim_graph.get_mut(run_forward_anim.node_index).unwrap();
            run_forward_graph_node.mask = BlendMask::UPPER_BODY.bits();
            println!("run forward mask: {}", BlendMask::UPPER_BODY.bits());
            animation_player.play(chop_animation.node_index).repeat();
            animation_player.play(run_forward_anim.node_index);
            let node_index = run_forward_anim.node_index;
            let clip_handle = run_forward_anim.clip_handle.clone();
            nif_animator.active_animations.insert(
                "runforward2w".to_string(),
                ActiveAnimation {
                    clip_handle,
                    node_index,
                    loop_count: 0,
                    blend_mask: BlendMask::empty(),
                    next_clip_name: Some("runforward2w_loop".to_string()),
                    priorities: [Priority::Hit; 4],
                    speed_mult: 1.0,
                },
            );
        }
    }
}
