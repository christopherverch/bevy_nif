use bevy::prelude::*;
use bevy_nif::nif_animation::NifAnimator;
use tes3mp_rust::{
    actor_defs::{ActorState, AnimationDisplacement, JumpingStateKind, RootMotionState},
    defs::{CreatureStats, IdToEntityMap, IsThirdPerson, LocalPlayer, MeshEntityLink},
};

use crate::{
    animation::animation_helpers::get_anim_components_for_creature, const_defs::MW_SCALE_REDUCTION,
};

pub fn calculate_animation_displacement_system(
    is_third_person: Res<IsThirdPerson>,

    mut creature_mesh_query: Query<&mut RootMotionState>,

    mut actor_query: Query<
        (
            &ActorState,
            &CreatureStats,
            &mut AnimationDisplacement,
            &MeshEntityLink,
        ),
        With<LocalPlayer>,
    >,
    mut animator_q: Query<(
        &mut AnimationPlayer,
        &mut NifAnimator,
        &AnimationGraphHandle,
    )>,
    id_to_entity_map: Res<IdToEntityMap>,
) {
    // This system only matters in third-person view.
    if !is_third_person.0 {
        // In first-person view, root motion is not applied. We explicitly clear
        // any existing animation displacement to prevent it from affecting the camera.
        if let Ok((_, _, mut animation_displacement, _mesh_entity_link)) = actor_query.single_mut()
        {
            animation_displacement.0 = Vec3::ZERO;
        }
        return;
    }
    // We get a link to the player's mesh here
    let Ok((actor_state, creature_stats, mut animation_displacement, mesh_entity_link)) =
        actor_query.single_mut()
    else {
        return;
    };

    // Get the root motion state of the mesh
    let Ok(mut root_motion_state) = creature_mesh_query.get_mut(mesh_entity_link.0) else {
        return;
    };

    // Root motion is ignored while jumping/falling. The speed is zero.
    if actor_state.jumping.state != JumpingStateKind::None {
        animation_displacement.0 = Vec3::ZERO;
        return;
    }

    let Some((_, animation_player, animator, _)) =
        get_anim_components_for_creature(&creature_stats, &id_to_entity_map, &mut animator_q)
    else {
        return;
    };

    let mut frame_displacement = Vec3::ZERO;
    let current_move_group = &actor_state.movement.current_animation_group;

    if !current_move_group.is_empty() {
        if let Some(definition) = animator.animation_definitions.get(current_move_group) {
            if let Some(curve) = &definition.root_translation_curve {
                if let Some(anim_state) = animation_player.animation(definition.node_index) {
                    let current_time = anim_state.elapsed() % definition.duration;

                    if current_move_group != &root_motion_state.last_animation_group
                        || current_time < root_motion_state.previous_time
                    {
                        root_motion_state.previous_time = current_time;
                    }
                    // Calculate the displacement delta since the last frame, handling animation loops correctly.
                    let delta = if current_time < root_motion_state.previous_time {
                        // The animation has looped since the last frame. To get the correct
                        // displacement, we must calculate it in two parts:

                        // 1. The displacement from the previous frame's timestamp to the end of the animation.
                        let pos_at_end_opt = curve.sample(definition.duration);
                        let pos_at_prev_opt = curve.sample(root_motion_state.previous_time);
                        let delta1 = pos_at_end_opt
                            .zip(pos_at_prev_opt)
                            .map_or(Vec3::ZERO, |(end, prev)| end - prev);

                        // 2. The displacement from the beginning of the animation to the new, smaller timestamp.
                        let pos_at_start_opt = curve.sample(0.0);
                        let pos_at_now_opt = curve.sample(current_time);
                        let delta2 = pos_at_start_opt
                            .zip(pos_at_now_opt)
                            .map_or(Vec3::ZERO, |(start, now)| now - start);

                        delta1 + delta2
                    } else {
                        // Normal frame, no loop.
                        let pos_now_opt = curve.sample(current_time);
                        let pos_prev_opt = curve.sample(root_motion_state.previous_time);
                        pos_now_opt
                            .zip(pos_prev_opt)
                            .map_or(Vec3::ZERO, |(now, prev)| now - prev)
                    };

                    let scaled_delta = (delta / MW_SCALE_REDUCTION) * anim_state.speed();
                    // Transform from NIF's coordinate system (Y-up, X-right) to Bevy's (Y-up, -Z-forward).
                    // The vertical (Y) component of root motion is discarded
                    // since the physics system handles y-motion with gravity
                    // TODO:: check if openmw flattens the y (scaled_delta.z) to 0.0
                    frame_displacement = Vec3::new(scaled_delta.x, 0.0, -scaled_delta.y);

                    root_motion_state.previous_time = current_time;
                    root_motion_state.last_animation_group = current_move_group.clone();
                }
            }
        }
    }
    // The final calculated displacement for this frame is stored in the AnimationDisplacement component.
    // This value is consumed by `prepare_character_velocity_system`, which converts it into a
    // velocity before passing it to the movement solver.
    animation_displacement.0 = frame_displacement;
}
