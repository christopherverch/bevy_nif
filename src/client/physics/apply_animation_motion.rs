use bevy::prelude::*;
use bevy_nif::nif_animation::NifAnimator;
use cdda_bevy::client::{
    actor_defs::{ActorState, AnimationDisplacement, JumpingStateKind, RootMotionState},
    animation::animation_helpers::get_anim_components_for_creature,
    defs::{CreatureStats, IdToEntityMap, IsThirdPerson, LocalPlayer, RootEntityLink},
};

use cdda_bevy::client::const_defs::MW_SCALE_REDUCTION;
pub fn calculate_animation_displacement_system(
    is_third_person: Res<IsThirdPerson>,

    mut creature_mesh_query: Query<(&mut RootMotionState, &RootEntityLink)>,

    mut actor_query: Query<(&ActorState, &mut AnimationDisplacement), With<LocalPlayer>>,
    mut animator_q: Query<(
        &mut AnimationPlayer,
        &mut NifAnimator,
        &AnimationGraphHandle,
    )>,
    creature_stats_q: Query<&CreatureStats>,
    id_to_entity_map: Res<IdToEntityMap>,
) {
    // This system only matters in third-person view.
    if !is_third_person.0 {
        // In first person, we clear the speed, not sure if this is necessary
        if let Ok((_, mut animation_displacement)) = actor_query.single_mut() {
            animation_displacement.0 = Vec3::ZERO;
        }
        return;
    }

    // TODO:: probably want a link to the mesh on the player instead of the reverse
    for (mut root_motion_state, root_entity_link) in creature_mesh_query.iter_mut() {
        let Some(root_entity) = root_entity_link.0 else {
            continue;
        };

        // Get the actor's state and the new component we need to write to.
        if let Ok((actor_state, mut animation_displacement)) = actor_query.get_mut(root_entity) {
            // Root motion is ignored while jumping/falling. The speed is zero.
            if actor_state.jumping.state != JumpingStateKind::None {
                animation_displacement.0 = Vec3::ZERO;
                continue;
            }

            let Ok(creature_stats) = creature_stats_q.get(root_entity) else {
                continue;
            };

            let Some((_, player, animator, _)) = get_anim_components_for_creature(
                &creature_stats,
                &id_to_entity_map,
                &mut animator_q,
            ) else {
                continue;
            };

            let mut frame_displacement = Vec3::ZERO;
            let current_move_group = &actor_state.movement.current_animation_group;

            if !current_move_group.is_empty() {
                if let Some(definition) = animator.animation_definitions.get(current_move_group) {
                    if let Some(curve) = &definition.root_translation_curve {
                        if let Some(anim_state) = player.animation(definition.node_index) {
                            let current_time = anim_state.elapsed() % definition.duration;

                            if current_move_group != &root_motion_state.last_animation_group
                                || current_time < root_motion_state.previous_time
                            {
                                root_motion_state.previous_time = current_time;
                            }

                            // This logic to get the displacement 'delta' is identical to your old code.
                            let delta = if current_time < root_motion_state.previous_time {
                                // A loop occurred.
                                let pos_at_end_opt = curve.sample(definition.duration);
                                let pos_at_prev_opt = curve.sample(root_motion_state.previous_time);
                                let delta1 = pos_at_end_opt
                                    .zip(pos_at_prev_opt)
                                    .map_or(Vec3::ZERO, |(end, prev)| end - prev);

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
                            // The NIF's local Y-axis is our "forward" Z-axis.
                            frame_displacement = Vec3::new(scaled_delta.x, 0.0, -scaled_delta.y);

                            root_motion_state.previous_time = current_time;
                            root_motion_state.last_animation_group = current_move_group.clone();
                        }
                    }
                }
            }

            // --- THIS IS THE NEW OUTPUT ---
            // Instead of applying the delta, we convert it to a speed and write it to the component.
            animation_displacement.0 = frame_displacement;
        }
    }
}
