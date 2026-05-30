use bevy::camera_controller::free_camera::FreeCameraPlugin;
// This example runs animations and is meant to be run with base_anim.nif and the B_N dark elf nif
// files inside of assets/data, but any can be loaded by replacing the names in setup.rs
use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_nif::nif_animation::bevy_types::{ActiveAnimation, AnimType, Priority};
use bevy_nif::nif_animation::{BlendMask, NifAnimator, SkeletonMap};
use bevy_nif::*;
use bevy_rapier3d::plugin::{NoUserData, RapierPhysicsPlugin};
use bevy_rapier3d::render::RapierDebugRenderPlugin;
mod setup;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BevyNifPlugin)
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(1.0, 0.8, 0.6),
            brightness: 100.0,
            affects_lightmapped_meshes: true,
        })
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugins(RapierDebugRenderPlugin::default())
        .add_plugins(FreeCameraPlugin)
        .add_systems(Startup, setup::setup_scene)
        .add_systems(Startup, setup::setup)
        .add_systems(Update, test_animations)
        .add_systems(Update, wireframe)
        .add_systems(Update, test_loop_anims)
        .add_plugins(EguiPlugin::default())
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
            let Some(runforward_clip) = animation_clips.get(&run_forward_anim.clip_handle) else {
                return;
            };
            let duration = runforward_clip.duration();
            let elapsed = {
                let Some(animation) = animation_player.animation(run_forward_anim.node_index)
                else {
                    return;
                };
                animation.elapsed()
            };
            println!("duration: {}, elapsed: {}", duration, elapsed,);
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
                println!("looping animation");
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
                    blend_mask: BlendMask::empty(),
                    next_clip_name: Some("runforward2w_loop".to_string()),
                    priorities: [Priority::Hit; 4],
                    speed_mult: 1.0,
                    anim_type: AnimType::Loop,
                    next_should_loop: false,
                    auto_remove: true,
                },
            );
        }
    }
}
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
