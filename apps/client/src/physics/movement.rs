use std::collections::HashSet;

use crate::defs::{ColliderEntityLink, LocalPlayer};
use avian3d::prelude::{
    Collider, CollisionLayers, Collisions, ContactPairFlags, ShapeCastConfig, ShapeHitData,
    SpatialQuery, SpatialQueryFilter,
};
use common::ActorState;
use mwphysics::{Actor, ActorTracer, CollisionShapeType, PtrHolder, trace_down};

use bevy::prelude::*;

use super::{components::CreaturePhysicsState, helpers::reject, unstuck::unstuck_character};
use super::{constants, helpers::project};
#[derive(Debug, Clone, Copy)]
pub struct SweepHit {
    pub entity: Entity,
    pub toi: f32,
    pub world_normal: Vec3,
    pub point: Vec3,
}
pub fn init_position(actor_q: Query<&mut Actor>) {
    for mut actor in actor_q {
        actor.apply_offset_change();
    }
}
pub fn character_movement_system(
    time: Res<Time<Fixed>>,
    mut character_query: Query<
        (
            Entity,
            &mut Transform,
            &mut CreaturePhysicsState,
            &ColliderEntityLink,
        ),
        With<LocalPlayer>,
    >,
    spatial_query: &SpatialQuery,
    colliders_q: Query<(&Transform, &Collider)>,
    collisions: Collisions,
    actor_q: Query<&ActorState>,
    collision_layers_q: Query<&CollisionLayers>,
) {
    let delta_time = time.delta_secs();
    if delta_time == 0.0 {
        return;
    }

    let actor_entities: HashSet<Entity> = character_query.iter().map(|(e, _, _, _)| e).collect();
    for (root_entity, mut transform, mut controller, collider_link) in character_query.iter_mut() {
        unstuck_character(
            &spatial_query,
            &colliders_q,
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

            let sweep_config = ShapeCastConfig {
                max_distance: move_vector.length(),
                compute_contact_on_penetration: true,
                ignore_origin_penetration: false, //
                ..default()
            };
            let move_dir = Dir3::new_unchecked(move_vector.normalize());
            dbg!("ignoring", root_entity);
            let sweep_result = spatial_query.cast_shape(
                character_shape,
                current_pos,
                Quat::IDENTITY,
                move_dir,
                &sweep_config,
                &SpatialQueryFilter::from_excluded_entities([root_entity]),
            );
            dbg!(sweep_result);

            if let Some(hit) = sweep_result {
                let travel_dist = move_vector.length();
                let normalized_toi = if travel_dist > 0.0 {
                    hit.distance / travel_dist
                } else {
                    0.0
                };
                let mut sweep_hit = SweepHit {
                    entity: hit.entity,
                    toi: normalized_toi,
                    // In Avian, hit.normal is the normal at the hit point
                    world_normal: hit.normal1,
                    point: hit.point1,
                };
                // Actor collision
                if actor_q.get(hit.entity).is_ok() {
                    if let Ok((hit_transform, _)) = colliders_q.get(hit.entity) {
                        // Check for existing penetration before overriding the collision normal.
                        if let Some(contact_pair) = collisions.get(root_entity, hit.entity) {
                            if contact_pair.flags.contains(ContactPairFlags::TOUCHING) {
                                let my_origin = current_pos; // Use current loop position
                                let other_origin = hit_transform.translation;
                                // Calculate horizontal push-out vector
                                let mut horizontal_normal = my_origin - other_origin;
                                horizontal_normal.y = 0.0;
                                horizontal_normal = horizontal_normal.normalize_or_zero();

                                // Check if moving towards the other actor
                                if horizontal_normal.dot(move_vector) < 0.0 {
                                    // Overriding the hit data to force us to "slide" away from the
                                    // actor we are colliding with
                                    sweep_hit.world_normal = horizontal_normal;
                                    sweep_hit.toi = 0.0; // In Avian logic, this means distance = 0
                                } else {
                                    // If moving away from an overlapping actor, ignore the collision
                                    current_pos += move_vector;
                                    remaining_time = 0.0;
                                    continue;
                                }
                            }
                            // If not overlapping, proceed with the normal sweep_hit result.
                        }
                    }
                }

                let was_stepped = try_step_up(
                    &spatial_query,
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
                let ejection_dist = ejection_vector.length();
                let ejection_dir = Dir3::new_unchecked(ejection_vector.normalize_or_zero());
                let ejection_config = ShapeCastConfig {
                    max_distance: ejection_dist,
                    compute_contact_on_penetration: true,
                    ignore_origin_penetration: false,
                    ..default()
                };
                if let Some(hit) = spatial_query.cast_shape(
                    character_shape,
                    current_pos,
                    transform.rotation,
                    ejection_dir,
                    &ejection_config,
                    &SpatialQueryFilter::from_excluded_entities([root_entity]),
                ) {
                    // If we hit something while trying to eject (e.g., a corner or narrow gap),
                    // move only as far as the hit allows.
                    // Avian returns hit.distance in world units.
                    current_pos += ejection_dir * (hit.distance * 0.5);
                } else {
                    // No obstruction, apply half the ejection vector as planned
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
                            let direction = Dir3::new_unchecked(ejection_vec.normalize_or_zero());
                            let distance = ejection_vec.length();
                            let ejection_hit = spatial_query.cast_shape(
                                character_shape,
                                current_pos,
                                Quat::IDENTITY,
                                direction,
                                &ShapeCastConfig {
                                    max_distance: distance,
                                    ignore_origin_penetration: false, // Don't ignore the very thing we are stuck in!
                                    ..default()
                                },
                                &SpatialQueryFilter::from_excluded_entities([root_entity]),
                            );
                            if let Some(hit) = ejection_hit {
                                // If the path is partially blocked, move as far as the hit allows (halved)
                                current_pos += direction * (hit.distance * 0.5);
                            } else {
                                // Path is clear, apply half the intended nudge
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
            let mut ground_tracer = ActorTracer::default();
            let down_target = current_pos + Vec3::NEG_Y * trace_dist;

            let actor = Actor::new(
                PtrHolder::new(root_entity, transform.translation),
                // TODO:: all the below args
                false,
                true,
                CollisionShapeType::Aabb,
                true,
                Vec3::ZERO,
                Vec3::ONE,
            );
            ground_tracer.find_ground(
                root_entity,
                current_pos,
                down_target,
                &spatial_query,
                &colliders_q,
                &collision_layers_q,
            );
            if ground_tracer.fraction < 1.0 {
                let max_slope_cos = constants::MAX_SLOPE_DEGREES.to_radians().cos();
                let hit_is_walkable = ground_tracer.plane_normal.y > max_slope_cos;
                if hit_is_walkable {
                    final_on_ground = true;
                    current_pos.y = ground_tracer.end_pos.y + constants::GROUND_OFFSET;
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

fn can_step_down(hit_normal: Vec3) -> bool {
    // 1. Check if we hit anything at all.
    // If trace_down returns Vec3::ZERO on a miss, we can't step down.
    if hit_normal == Vec3::ZERO {
        return false;
    }

    // 2. Slope check
    let max_slope_cos = constants::MAX_SLOPE_DEGREES.to_radians().cos();

    // hit_normal.y is the "steepness."
    // 1.0 is flat ground, 0.0 is a vertical wall.
    hit_normal.y > max_slope_cos
}
fn try_step_up(
    spatial_query: &SpatialQuery,
    entity: Entity,
    character_shape: &Collider,
    current_pos: &mut Vec3,
    current_rotation: Quat,
    current_velocity: &mut Vec3,
    remaining_time: &mut f32,
    controller: &mut CreaturePhysicsState,
    first_iteration: bool,
    colliders_q: &Query<(&Transform, &Collider)>,
    collision_layers_q: &Query<&CollisionLayers>,
    actor: &mut Actor,
) -> bool {
    if current_velocity.xz().length_squared() == 0.0 {
        return false;
    }

    let filter = SpatialQueryFilter::from_excluded_entities([entity]);
    let step_height = constants::STEP_SIZE_UP;
    let up_direction = Dir3::new_unchecked(Vec3::Y);
    // 1. Upward Cast
    let up_cast_vector = Vec3::Y * constants::STEP_SIZE_UP;
    let up_sweep_config = ShapeCastConfig {
        max_distance: step_height,
        // IMPORTANT: We set this to true so we don't get stuck on the floor
        // we are currently standing on while trying to cast upward.
        ignore_origin_penetration: true,
        compute_contact_on_penetration: true,
        ..default()
    };

    let up_hit = spatial_query.cast_shape(
        character_shape,
        *current_pos,
        current_rotation,
        up_direction,
        &up_sweep_config,
        &filter,
    );
    let up_distance = if let Some(hit) = up_hit {
        let dist = hit.distance;
        if dist <= constants::COLLISION_MARGIN {
            return false;
        }
        dist - constants::COLLISION_MARGIN
    } else {
        constants::STEP_SIZE_UP
    };

    let elevated_base_pos = *current_pos + Vec3::Y * up_distance;

    // 2. Multi-Attempt Forward/Down Trace Loop
    // A small helper struct to hold the results of a valid step
    struct StepResult {
        final_pos: Vec3,
        ground_normal: Vec3,
        forward_hit: Option<ShapeHitData>,
    }
    let mut successful_step_info: Option<StepResult> = None;
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
        let forward_distance = forward_cast_vector.length();
        // Early out if movement is effectively zero
        if forward_distance < 0.00001 {
            break;
        }
        let forward_direction = Dir3::new_unchecked(forward_cast_vector / forward_distance);

        let forward_hit = spatial_query.cast_shape(
            character_shape,
            elevated_base_pos,
            current_rotation,
            forward_direction,
            &ShapeCastConfig {
                max_distance: forward_distance,
                // Use false here, we want to hit the wall/stair riser if it's there
                ignore_origin_penetration: false,
                compute_contact_on_penetration: true,
                ..default()
            },
            &filter,
        );
        // --- MODIFIED BLOCK: This now includes the ejection logic from the C++ source. ---
        let (step_forward_pos, forward_hit_details) = if let Some(hit) = forward_hit {
            let dist = hit.distance;
            if dist <= constants::COLLISION_MARGIN {
                continue; // Not enough space to move, try next attempt.
            }
            let final_dist = dist - constants::COLLISION_MARGIN;
            let pre_ejection_pos = elevated_base_pos + move_direction * final_dist;

            // Safely eject from the wall we hit to avoid penetration before the downward trace.
            let forward_hit_normal = hit.normal1;

            let ejection_cast_vector = forward_hit_normal * constants::COLLISION_MARGIN * 2.0;
            let ejection_dist_max = ejection_cast_vector.length();
            let ejection_dir = Dir3::new_unchecked(ejection_cast_vector / ejection_dist_max);
            let ejection_hit = spatial_query.cast_shape(
                character_shape,
                pre_ejection_pos,
                current_rotation,
                ejection_dir,
                &ShapeCastConfig {
                    max_distance: ejection_dist_max,
                    ignore_origin_penetration: false,
                    ..default()
                },
                &filter,
            );
            let final_step_forward_pos = if let Some(ejection_details) = ejection_hit {
                // We hit something during our ejection trace.
                if hit.distance > constants::COLLISION_MARGIN {
                    // Calculate how much space beyond the first margin we actually have.
                    let space_available = hit.distance - constants::COLLISION_MARGIN;

                    // Push out by that available space (up to one full COLLISION_MARGIN).
                    pre_ejection_pos + (forward_hit_normal * space_available)
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
            actor,
            *current_pos,
            up_distance + constants::STEP_SIZE_DOWN,
            spatial_query,
            colliders_q,
            collision_layers_q,
        );

        if actor.on_ground && !actor.on_slope {
            successful_step_info = Some(StepResult {
                final_pos: down_pos,
                ground_normal: down_normal,
                forward_hit: forward_hit_details,
            });
            break;
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
