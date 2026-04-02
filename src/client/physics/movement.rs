use std::collections::HashSet;

use tes3mp_rust::{
    actor_defs::ActorState,
    defs::{ColliderEntityLink, LocalPlayer},
};

use bevy::prelude::*;
use bevy_rapier3d::{
    plugin::ReadRapierContext,
    prelude::{
        Collider, QueryFilter, RapierContext, ShapeCastHit, ShapeCastHitDetails, ShapeCastOptions,
    },
};

use super::{components::CreaturePhysicsState, helpers::reject, unstuck::unstuck_character};
use super::{constants, helpers::project};
#[derive(Debug, Clone, Copy)]
pub struct SweepHit {
    pub entity: Entity,
    pub toi: f32,
    pub world_normal: Vec3,
    pub point: Vec3,
}

pub fn character_movement_system(
    time: Res<Time<Fixed>>,
    rapier_context: ReadRapierContext,
    mut character_query: Query<
        (
            Entity,
            &mut Transform,
            &mut CreaturePhysicsState,
            &ColliderEntityLink,
        ),
        With<LocalPlayer>,
    >,

    collider_query: Query<(
        &GlobalTransform,
        &Collider,
        Option<&ActorState>, // Use ActorState to identify actors
    )>,
) {
    let delta_time = time.delta_secs();
    if delta_time == 0.0 {
        return;
    }

    let actor_entities: HashSet<Entity> = character_query.iter().map(|(e, _, _, _)| e).collect();
    let Ok(rapier_context) = rapier_context.single() else {
        return;
    };
    for (root_entity, mut transform, mut controller, collider_link) in character_query.iter_mut() {
        unstuck_character(
            &rapier_context,
            &collider_query,
            root_entity,
            collider_link.0,
            &mut transform,
            &mut controller,
        );
        let mut remaining_time = delta_time;
        let mut iterations = 0;
        let mut current_pos = transform.translation;
        let mut current_velocity = controller.velocity_for_frame;
        controller.times_slid = 0;
        let character_shape = &controller.collision_shape.clone();
        let on_ground_this_frame = controller.on_ground;

        while remaining_time > 0.0 && iterations < constants::MAX_ITERATIONS {
            iterations += 1;
            let move_vector = current_velocity * remaining_time;

            if move_vector.length_squared() < 0.0001 {
                break;
            }

            let sweep_options = ShapeCastOptions {
                max_time_of_impact: 1.0,
                stop_at_penetration: true,
                ..default()
            };
            dbg!("ignoring", root_entity);
            let sweep_result = rapier_context.cast_shape(
                current_pos,
                Quat::IDENTITY,
                move_vector,
                character_shape.into(),
                sweep_options, // Pass the struct here
                QueryFilter::new().exclude_rigid_body(root_entity),
            );
            dbg!(sweep_result);

            if let Some((hit_entity, hit)) = sweep_result {
                let mut sweep_hit = SweepHit {
                    entity: hit_entity,
                    toi: hit.time_of_impact,
                    world_normal: if let Some(ShapeCastHitDetails {
                        witness1: _witness1,
                        witness2: _witness2,
                        normal1,
                        normal2: _normal2,
                    }) = hit.details
                    {
                        normal1.into()
                    } else {
                        -move_vector.normalize_or_zero()
                    },
                    point: current_pos + move_vector * hit.time_of_impact,
                };
                // Actor collision
                if let Ok((hit_transform, _, Some(_))) = collider_query.get(hit_entity) {
                    // Check for existing penetration before overriding the collision normal.
                    if rapier_context
                        .contact_pair(root_entity, hit_entity)
                        .is_some()
                    {
                        let my_origin = transform.translation;
                        let other_origin = hit_transform.translation();

                        // Calculate horizontal push-out vector
                        let mut horizontal_normal = my_origin - other_origin;
                        horizontal_normal.y = 0.0; // In Bevy, Y is vertical.
                        horizontal_normal = horizontal_normal.normalize_or_zero();

                        // Only apply the override if moving towards the other actor.
                        if horizontal_normal.dot(move_vector) < 0.0 {
                            sweep_hit.world_normal = horizontal_normal;
                            sweep_hit.toi = 0.0; // Force immediate collision response
                        } else {
                            // If moving away from an overlapping actor, ignore the collision entirely
                            // to prevent getting stuck.
                            current_pos += move_vector;
                            remaining_time = 0.0;
                            continue;
                        }
                    }
                    // If not overlapping, proceed with the normal sweep_hit result.
                }

                let was_stepped = try_step_up(
                    &rapier_context,
                    root_entity,
                    character_shape,
                    &mut current_pos,
                    &mut current_velocity,
                    &mut remaining_time,
                    &mut controller,
                    iterations == 1,
                );

                if was_stepped {
                    // If a step-up was successfully performed, the character's position has changed.
                    // We restart the movement loop from this new position to re-evaluate the next move.
                    continue;
                }

                let dist_to_hit = sweep_hit.toi * move_vector.length();
                let safe_move_dist = (dist_to_hit - constants::COLLISION_MARGIN).max(0.0);
                current_pos += move_vector.normalize_or_zero() * safe_move_dist;

                let ejection_vector = sweep_hit.world_normal * constants::COLLISION_MARGIN * 2.0;
                if let Some((_, ejection_hit)) = rapier_context.cast_shape(
                    current_pos,
                    Quat::IDENTITY,
                    ejection_vector,
                    character_shape.into(),
                    sweep_options, // Re-use the same options
                    QueryFilter::new().exclude_rigid_body(root_entity),
                ) {
                    current_pos += ejection_vector * (ejection_hit.time_of_impact * 0.5);
                } else {
                    current_pos += ejection_vector * 0.5;
                }

                // (The rest of the slide logic is unchanged)
                let time_fraction_consumed = (safe_move_dist / move_vector.length()).min(1.0);
                remaining_time *= 1.0 - time_fraction_consumed;
                let mut used_seam_logic = false;
                let orig_plane_normal = sweep_hit.world_normal;
                let max_slope_cos = constants::MAX_SLOPE_DEGREES.to_radians().cos();
                let hit_is_walkable = orig_plane_normal.y > max_slope_cos;
                let seen_ground_this_iteration =
                    (on_ground_this_frame && !controller.on_slope) || hit_is_walkable;

                // --- Crevice/Seam Logic ---
                // When sliding into an acute interior corner, rejecting from each wall in succession
                // can cancel out movement and make the character feel "stuck". This logic detects
                // when we hit a surface that is acute to our last slide plane. If so, it calculates
                // the "seam" of the two planes and projects velocity along that seam, allowing the
                // character to slide smoothly along the corner.
                if controller.times_slid > 0 {
                    // Check for an acute angle with the last two slide planes.
                    let dot_a = orig_plane_normal.dot(controller.last_slide_normal);

                    // The fallback normal is only relevant after the second slide.
                    let dot_b = if controller.times_slid > 1 {
                        orig_plane_normal.dot(controller.last_slide_normal_fallback)
                    } else {
                        1.0 // A non-negative value to ensure it's not chosen by default.
                    };

                    if dot_a <= 0.0 || dot_b <= 0.0 {
                        // Choose the normal from the plane that is more acute to the current one.
                        let best_normal = if dot_b < dot_a {
                            let fallback = controller.last_slide_normal_fallback;
                            // If we use the fallback, it becomes the new "last" normal for this calculation.
                            controller.last_slide_normal = fallback;
                            fallback
                        } else {
                            controller.last_slide_normal
                        };

                        // The seam is the cross product of the two normals.
                        let seam_vector = best_normal.cross(orig_plane_normal).normalize_or_zero();

                        if seam_vector.length_squared() > 0.001 {
                            // Project the velocity onto the seam vector to slide along it.
                            current_velocity = project(current_velocity, seam_vector);

                            // Also apply a small positional ejection out of the crevice to prevent snagging.
                            let average_normal =
                                (best_normal + orig_plane_normal).normalize_or_zero();
                            let ejection_vec = average_normal * (constants::COLLISION_MARGIN * 2.0);

                            if let Some((_, ejection_hit)) = rapier_context.cast_shape(
                                current_pos,
                                Quat::IDENTITY,
                                ejection_vec,
                                character_shape.into(),
                                sweep_options,
                                QueryFilter::new().exclude_rigid_body(root_entity),
                            ) {
                                // Move halfway into the clear space found.
                                current_pos += ejection_vec * (ejection_hit.time_of_impact * 0.5);
                            } else {
                                current_pos += ejection_vec * 0.5;
                            }

                            used_seam_logic = true;
                        }
                    }
                }
                if !used_seam_logic {
                    let old_velocity = current_velocity;
                    let mut effective_normal = orig_plane_normal;

                    if seen_ground_this_iteration
                        && !hit_is_walkable
                        && effective_normal.y.abs() > 0.001
                    {
                        effective_normal.y = 0.0;
                        effective_normal = effective_normal.normalize_or_zero();
                    }
                    current_velocity = reject(current_velocity, effective_normal);

                    if !hit_is_walkable {
                        current_velocity.y = current_velocity.y.min(old_velocity.y);
                    }
                }

                controller.last_slide_normal_fallback = controller.last_slide_normal;
                controller.last_slide_normal = orig_plane_normal;
                controller.times_slid += 1;
            } else {
                current_pos += move_vector;
                remaining_time = 0.0;
            }
        }

        // --- Final Ground Check & State Update (Unchanged) ---
        let mut final_on_ground = false;
        let mut final_on_slope = false;
        if controller.inertia.y <= 0.0 {
            let trace_dist = constants::GROUND_OFFSET * 2.0
                + if on_ground_this_frame {
                    constants::STEP_SIZE_DOWN
                } else {
                    0.0
                };
            if let Some(hit) = trace_down(
                &rapier_context,
                root_entity,
                character_shape,
                current_pos,
                trace_dist,
                &actor_entities,
            ) {
                let max_slope_cos = constants::MAX_SLOPE_DEGREES.to_radians().cos();
                let hit_is_walkable = hit.world_normal.y > max_slope_cos;
                if hit_is_walkable {
                    final_on_ground = true;
                    current_pos.y = hit.point.y + constants::GROUND_OFFSET;
                } else {
                    final_on_slope = true;
                }
            }
        }

        if !final_on_ground {
            controller.inertia.y -= constants::GRAVITY_CONST * delta_time;
        } else {
            controller.inertia = Vec3::ZERO;
        }
        controller.on_ground = final_on_ground;
        controller.on_slope = final_on_slope;
        transform.translation = current_pos;
    }
}

/// A downward trace to find the ground beneath the character.
fn trace_down(
    rapier_context: &RapierContext,
    character_entity: Entity,
    character_shape: &Collider,
    position: Vec3,
    max_dist: f32,
    actor_entities: &HashSet<Entity>,
) -> Option<SweepHit> {
    let down_vector = Vec3::NEG_Y * max_dist;
    let predicate = |e| !actor_entities.contains(&e);
    let filter = QueryFilter::new()
        .exclude_rigid_body(character_entity)
        .predicate(&predicate);
    let sweep_options = ShapeCastOptions {
        max_time_of_impact: 1.0,
        stop_at_penetration: true,
        ..default()
    };
    let sweep_result = rapier_context.cast_shape(
        position,
        Quat::IDENTITY,
        down_vector,
        character_shape.into(),
        sweep_options,
        filter,
    );

    if let Some((hit_entity, hit)) = sweep_result {
        return Some(SweepHit {
            entity: hit_entity,
            toi: hit.time_of_impact,
            world_normal: if let Some(ShapeCastHitDetails {
                witness1: _witness1,
                witness2: _witness2,
                normal1,
                normal2: _normal2,
            }) = hit.details
            {
                normal1.into()
            } else {
                Vec3::Y
            },
            point: position + down_vector * hit.time_of_impact,
        });
    }

    None
}

fn can_step_down(hit: &SweepHit) -> bool {
    let max_slope_cos = constants::MAX_SLOPE_DEGREES.to_radians().cos();
    hit.world_normal.y > max_slope_cos
}

fn try_step_up(
    rapier_context: &RapierContext,
    entity: Entity,
    character_shape: &Collider,
    current_pos: &mut Vec3,
    current_velocity: &mut Vec3,
    remaining_time: &mut f32,
    controller: &mut CreaturePhysicsState,
    first_iteration: bool,
) -> bool {
    if current_velocity.xz().length_squared() == 0.0 {
        return false;
    }

    let filter = QueryFilter::new().exclude_rigid_body(entity);
    let sweep_options = ShapeCastOptions {
        max_time_of_impact: 1.0,
        stop_at_penetration: true,
        ..default()
    };

    // 1. Upward Cast
    let up_cast_vector = Vec3::Y * constants::STEP_SIZE_UP;
    let up_hit = rapier_context.cast_shape(
        *current_pos,
        Quat::IDENTITY,
        up_cast_vector,
        character_shape.into(),
        sweep_options,
        filter,
    );

    let up_distance = if let Some((_, hit)) = up_hit {
        let dist = hit.time_of_impact * constants::STEP_SIZE_UP;
        if dist <= constants::COLLISION_MARGIN {
            return false;
        }
        dist - constants::COLLISION_MARGIN
    } else {
        constants::STEP_SIZE_UP
    };

    let elevated_base_pos = *current_pos + Vec3::Y * up_distance;

    // 2. Multi-Attempt Forward/Down Trace Loop
    let mut successful_step_info: Option<(SweepHit, Option<ShapeCastHit>)> = None;
    let move_direction = current_velocity.normalize_or_zero();

    for attempt in 1..=3 {
        let (forward_cast_vector, _is_fixed_step) = match attempt {
            1 => (*current_velocity * *remaining_time, false),
            2 => {
                if !constants::DO_EXTRA_STAIR_HACKS || !first_iteration {
                    break;
                }
                (move_direction * constants::S_MIN_STEP, true)
            }
            3 => {
                if !constants::DO_EXTRA_STAIR_HACKS || !first_iteration {
                    break;
                }
                (move_direction * constants::S_MIN_STEP_2, true)
            }
            _ => unreachable!(),
        };

        let forward_hit = rapier_context.cast_shape(
            elevated_base_pos,
            Quat::IDENTITY,
            forward_cast_vector,
            character_shape.into(),
            sweep_options,
            filter,
        );

        // --- MODIFIED BLOCK: This now includes the ejection logic from the C++ source. ---
        let (step_forward_pos, forward_hit_details) = if let Some((_, hit)) = forward_hit {
            let dist = hit.time_of_impact * forward_cast_vector.length();
            if dist <= constants::COLLISION_MARGIN {
                continue; // Not enough space to move, try next attempt.
            }
            let final_dist = dist - constants::COLLISION_MARGIN;
            let pre_ejection_pos = elevated_base_pos + move_direction * final_dist;

            // Safely eject from the wall we hit to avoid penetration before the downward trace.
            let forward_hit_normal = if let Some(details) = &hit.details {
                details.normal1.into()
            } else {
                -move_direction
            };

            let ejection_cast_vector = forward_hit_normal * constants::COLLISION_MARGIN * 2.0;

            let ejection_hit = rapier_context.cast_shape(
                pre_ejection_pos,
                Quat::IDENTITY,
                ejection_cast_vector,
                character_shape.into(),
                sweep_options,
                filter,
            );

            let final_step_forward_pos = if let Some((_, ejection_details)) = ejection_hit {
                // We hit something during our ejection trace.
                if ejection_details.time_of_impact > 0.5 {
                    // There's more than COLLISION_MARGIN of free space, so we can push out safely.
                    let effective_fraction = ejection_details.time_of_impact * 2.0 - 1.0;
                    pre_ejection_pos
                        + forward_hit_normal * constants::COLLISION_MARGIN * effective_fraction
                } else {
                    // Not enough free space, stay put.
                    pre_ejection_pos
                }
            } else {
                // Ejection trace hit nothing, so we have plenty of space. Eject by one margin.
                pre_ejection_pos + forward_hit_normal * constants::COLLISION_MARGIN
            };

            (final_step_forward_pos, Some(hit))
        } else {
            // Forward trace hit nothing.
            (elevated_base_pos + forward_cast_vector, None)
        };

        let actor_entities = HashSet::new();
        let down_hit = trace_down(
            rapier_context,
            entity,
            character_shape,
            step_forward_pos, // Use the new, potentially ejected position.
            up_distance + constants::STEP_SIZE_DOWN,
            &actor_entities,
        );

        if let Some(hit) = down_hit {
            if can_step_down(&hit) {
                successful_step_info = Some((hit, forward_hit_details));
                break;
            }
        }
    }

    // 3. Finalize Successful Step
    if let Some((down_hit, forward_hit_details)) = successful_step_info {
        let final_pos = down_hit.point;
        if (final_pos.y - current_pos.y) > up_distance && !controller.on_ground {
            return false;
        }
        *current_pos = final_pos + Vec3::Y * constants::GROUND_OFFSET;
        if let Some(hit) = forward_hit_details {
            if let Some(details) = hit.details {
                let mut plane_normal: Vec3 = details.normal1.into();
                if controller.on_ground
                    && plane_normal.y <= constants::MAX_SLOPE_DEGREES.to_radians().cos()
                    && plane_normal.y != 0.0
                {
                    plane_normal.y = 0.0;
                    plane_normal = plane_normal.normalize_or_zero();
                }
                *current_velocity = reject(*current_velocity, plane_normal);
            }
        }
        *current_velocity = reject(*current_velocity, down_hit.world_normal);

        let fraction_travelled = forward_hit_details.map_or(1.0, |h| h.time_of_impact);
        *remaining_time *= 1.0 - fraction_travelled;

        return true;
    }

    false
}
