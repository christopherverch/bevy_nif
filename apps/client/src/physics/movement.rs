use crate::defs::{ColliderEntityLink, LocalPlayer, Player, WorldFrameData};
use crate::mechanics::creature_stats::CreatureStats;
use crate::physics::helpers::get_pitch_yaw_3d_approx_movement;
use crate::physics::stepper::Stepper;
use avian3d::prelude::{Collider, CollisionLayers, SpatialQuery};
use common::consts::MW_UNITS_PER_METER;
use esm_parser_lib::RefId;
use esm_store::EsmStore;
use mwphysics::{
    ActorFrameData, ActorPhysics, ActorTracer, PositionInterpolation, is_walkable_slope,
};

use bevy::prelude::*;

use super::helpers::reject;
use super::{constants, helpers::project};

pub fn init_positions(
    actor_q: Query<(
        &ActorPhysics,
        &mut ActorFrameData,
        &mut PositionInterpolation,
        &Transform,
    )>,
) {
    for (actor, mut frame_data, mut position_interpolation, transform) in actor_q {
        frame_data.position = transform.translation;
        if frame_data.water_collision && frame_data.position.y < frame_data.water_level
        //&& actor->can_move_to_water_surface(..)
        {
            let offset = Vec3::new(0.0, frame_data.water_level - frame_data.position.y, 0.0);
            //  let move_to_active = false;
            //           move_object_by(actor_entity, offset, move_to_active, &mut transform);
            position_interpolation.current_position += offset;
            frame_data.position = position_interpolation.current_position;
        }
        frame_data.old_height = frame_data.position.y;
        let (yaw, pitch, _roll) = transform.rotation.to_euler(EulerRot::YXZ);
        frame_data.rotation = Vec2::new(pitch, yaw);
        frame_data.inertia = actor.force;
        frame_data.stuck_frames = actor.stuck_frames;
        frame_data.last_stuck_position = actor.last_stuck_position;
    }
}
/// Updates the position that the visuals will interpolate to, not the actual position
pub fn update_positions(
    mut actor_q: Query<(
        &mut PositionInterpolation,
        &mut ActorPhysics,
        &mut ActorFrameData,
    )>,
) {
    for (mut interpolation, mut actor_physics, mut actor_fd) in actor_q.iter_mut() {
        // Try to set the new physics position. If we had an offset or didn't move, apply the
        // offset and don't change our physics position otherwise.
        if actor_physics.set_position(&mut interpolation, actor_fd.position) {
            // Actor fd still gets updated, not sure why?
            // account for potential position change made by script
            actor_fd.position = interpolation.current_position;
        }
    }
}
pub fn interpolate_visual_positions(
    interpolation_q: Query<(&mut PositionInterpolation, &mut Transform)>,
    time: Res<Time>,
) {
    for (mut interpolation, mut transform) in interpolation_q {
        dbg!(transform.translation);
        transform.translation = interpolation.get_and_update_interpolated_pos(time.delta_secs());
    }
}
pub fn character_movement_system(
    time: Res<Time<Fixed>>,
    mut character_query: Query<
        (
            &mut ActorFrameData,
            &ColliderEntityLink,
            &mut PositionInterpolation,
        ),
        With<LocalPlayer>,
    >,
    spatial_query: SpatialQuery,
    colliders_q: Query<(&Transform, &Collider)>,
    actor_q: Query<&ActorPhysics>,
    collision_layers_q: Query<&CollisionLayers>,
    esm_store: Res<EsmStore>,
    world_frame_data: Res<WorldFrameData>,
) {
    let delta_time = time.delta_secs();

    for (mut actor_fd, collider_link, mut position_interpolation) in character_query.iter_mut() {
        position_interpolation.previous_position = actor_fd.position;
        actor_fd.walking_on_water = false;
        if actor_fd.skip_collision_detection {
            let dist_moved = get_pitch_yaw_3d_approx_movement(&actor_fd) * delta_time;
            actor_fd.position += dist_moved;
            continue;
        }
        // Adjust for collision mesh offset relative to actor's "location"
        // (doTrace doesn't take local/interior collision shape translation into account, so we have to do it on our
        // own) for compatibility with vanilla assets, we have to derive this from the vertical half extent instead of
        // from internal hull translation if not for this hack, the "correct" collision hull position would be
        // physicActor->getScaledMeshTranslation()
        // vanilla-accurate
        // might need to do +actor_fd.half_extents_y since the collision object is normally on the
        // feet, right now it's centered and does not do what openmw does
        actor_fd.position.y;

        let swim_level = actor_fd.swim_level;
        let mut actor_tracer = ActorTracer::default();
        let mut velocity;

        // Dead and paralyzed actors underwater will float to the surface,
        // if the CharacterController tells us to do so
        if actor_fd.movement.y > 0.0 && actor_fd.inert && actor_fd.position.y < swim_level {
            const WATER_UPWARDS_FLOAT_VELOCITY: Vec3 =
                Vec3::new(0.0, 25.0 / MW_UNITS_PER_METER, 0.0);
            velocity = WATER_UPWARDS_FLOAT_VELOCITY;
            // Swimming and flying have the same movement
        } else if actor_fd.position.y < swim_level || actor_fd.flying {
            velocity = get_pitch_yaw_3d_approx_movement(&actor_fd);
        } else {
            let yaw_rotation = Quat::from_rotation_y(actor_fd.rotation.y);
            velocity = yaw_rotation * actor_fd.movement;
            if (velocity.y > 0.0 && actor_fd.is_on_ground && !actor_fd.is_on_slope)
                || (velocity.y > 0.0
                    && velocity.y + actor_fd.inertia.y <= -velocity.y
                    && actor_fd.is_on_slope)
            {
                actor_fd.inertia = velocity;
            } else if !actor_fd.is_on_ground || actor_fd.is_on_slope {
                velocity += actor_fd.inertia;
            }
        }
        // Now that we have the effective movement vector, apply wind forces to it

        if world_frame_data.is_in_storm && velocity.length() > 0.0 {
            if let Some(storm_walk_mult_gmst) = esm_store
                .gamesettings
                .get(&RefId::from_str("fStromWalkMult"))
            {
                let storm_walk_mult = storm_walk_mult_gmst.value.0.as_float();
                let angle_cos = world_frame_data.storm_direction.dot(velocity) / velocity.length();
                velocity *= 1.0 + storm_walk_mult * angle_cos;
            }
        }
        let orig_velocity = velocity;
        let mut new_position = actor_fd.position;
        /*
         * A loop to find newPosition using tracer, if successful different from the starting position.
         * nextpos is the local variable used to find potential newPosition, using velocity and remainingTime
         * The initial velocity was set earlier (see above).
         */
        let mut remaining_time = delta_time;
        let mut num_times_slid = 0;
        let mut last_slide_normal = Vec3::Y;
        let mut last_slide_normal_fallback = Vec3::Y;
        let mut force_ground_test = false;
        let mut iterations = 0;
        let mut stepper = Stepper {
            collision_layers_q: &collision_layers_q,
            tracer: ActorTracer::default(),
            up_stepper: ActorTracer::default(),
            down_stepper: ActorTracer::default(),
        };
        while remaining_time > 0.0001 && iterations < constants::MAX_ITERATIONS {
            let next_pos = new_position + velocity * remaining_time;
            let underwater = new_position.y < swim_level;

            // 1. Water Surface Ceiling (Prevent swimming up into air)
            if !actor_fd.flying && next_pos.y > swim_level && underwater {
                velocity.y = 0.0;
                continue;
            }
            if new_position.distance_squared(next_pos) > 0.0001 {
                actor_tracer.do_trace(
                    collider_link.0,
                    new_position,
                    next_pos,
                    &spatial_query,
                    &colliders_q,
                    &collision_layers_q,
                );
                // check for obstructions
                if actor_tracer.fraction >= 1.0 {
                    new_position = actor_tracer.end_pos; // ok to move, set new_position
                    break;
                }
            } else {
                // The current position and next position are nearly the same, so just exit.
                // Since we aren't performing any collision detection, we want to reject the next
                // position, so that we don't slowly move inside another object.
                break;
            }
            let seen_ground = !actor_fd.flying
                && !underwater
                && ((actor_fd.is_on_ground && !actor_fd.is_on_slope)
                    || is_walkable_slope(actor_tracer.plane_normal));
            let hit_height = actor_tracer.hit_point.y - actor_tracer.end_pos.y;
            let old_position = new_position;
            let mut used_step_logic = false;
            let hit_actor = actor_tracer
                .hit_entity
                .map_or(false, |e| actor_q.get(e).is_ok());
            if !hit_actor {
                if hit_height < constants::STEP_SIZE_UP {
                    // Try to step up onto it.
                    // NOTE: this modifies newPosition and velocity on its own if successful
                    used_step_logic = stepper.step(
                        collider_link.0,
                        &mut new_position,
                        &mut velocity,
                        &mut remaining_time,
                        seen_ground,
                        iterations == 0,
                        &spatial_query,
                        &colliders_q,
                    );
                    // TODO:: if the object I hit has scriptedcollisiontype player or actor, trigger
                    // an event so that the script is notified that we hit the object
                }
            }
            if used_step_logic {
                if actor_fd.is_aquatic && new_position.y > actor_fd.water_level {
                    new_position = old_position;
                } else if !actor_fd.flying && actor_fd.position.y >= swim_level {
                    force_ground_test = true;
                }
            } else {
                // 1. Calculate remaining time based on how far we traveled before hitting
                remaining_time *= 1.0 - actor_tracer.fraction;

                let mut plane_normal = actor_tracer.plane_normal;
                let orig_plane_normal = plane_normal;

                // 2. Snag Prevention
                if seen_ground && !is_walkable_slope(plane_normal) && plane_normal.y != 0.0 {
                    plane_normal.y = 0.0;
                    plane_normal = plane_normal.normalize_or_zero();
                }

                // 3. Move to impact point with Collision Margin
                if new_position.distance_squared(actor_tracer.end_pos)
                    > constants::COLLISION_MARGIN.powi(2)
                {
                    let direction = velocity.normalize_or_zero();
                    new_position = actor_tracer.end_pos - (direction * constants::COLLISION_MARGIN);
                }
                // 4. Basic Slide Velocity
                let mut new_velocity = if velocity.dot(plane_normal) <= 0.0 {
                    reject(velocity, plane_normal)
                } else {
                    velocity
                };
                let mut used_seam_logic = false;

                // 5. Seam Logic (Acute Angle / Corner Sliding)
                if num_times_slid > 0 {
                    let dot_a = last_slide_normal.dot(orig_plane_normal);
                    let mut dot_b = last_slide_normal_fallback.dot(orig_plane_normal);

                    if num_times_slid <= 1 {
                        dot_b = 1.0; // Ignore fallback for first few slides
                    }

                    if dot_a <= 0.0 || dot_b <= 0.0 {
                        let mut best_normal = last_slide_normal;
                        if dot_b < dot_a {
                            best_normal = last_slide_normal_fallback;
                            last_slide_normal = last_slide_normal_fallback;
                        }

                        // Cross product gives the "seam" direction
                        let constraint_vector = best_normal.cross(orig_plane_normal);
                        if constraint_vector.length_squared() > 0.0 {
                            let constraint_vector = constraint_vector.normalize();
                            // Slide along the corner seam
                            new_velocity = project(velocity, constraint_vector);

                            // Adjust position to stay outside the crevice
                            let average_normal =
                                (best_normal + orig_plane_normal).normalize_or_zero();
                            actor_tracer.do_trace(
                                collider_link.0,
                                new_position,
                                new_position + average_normal * (constants::COLLISION_MARGIN * 2.0),
                                &spatial_query,
                                &colliders_q,
                                &collision_layers_q,
                            );
                            new_position = (new_position + actor_tracer.end_pos) / 2.0;
                            used_seam_logic = true;
                        }
                    }
                }
                // 6. Concave Geometry Ejection
                if !used_seam_logic {
                    actor_tracer.do_trace(
                        collider_link.0,
                        new_position,
                        new_position + plane_normal * (constants::COLLISION_MARGIN * 2.0),
                        &spatial_query,
                        &colliders_q,
                        &collision_layers_q,
                    );
                    new_position = (new_position + actor_tracer.end_pos) / 2.0;
                }

                // 7. Backward Movement Short-circuit (Fixes infinite bouncing)
                if seen_ground && new_velocity.dot(orig_velocity) <= 0.0 {
                    let perpendicular = new_velocity.cross(orig_velocity);
                    if perpendicular.length_squared() > 0.0 {
                        let perpendicular = perpendicular.normalize();
                        if perpendicular.y.abs() > 0.7071 {
                            break;
                        }
                    }
                }

                // 8. Prevent sliding up steep slopes while in the air (Gravity logic)
                if new_position.y >= swim_level
                    && !actor_fd.flying
                    && !is_walkable_slope(plane_normal)
                    && !used_seam_logic
                {
                    new_velocity.y = new_velocity.y.min(velocity.y);
                }

                // Update state for next iteration
                num_times_slid += 1;
                last_slide_normal_fallback = last_slide_normal;
                last_slide_normal = orig_plane_normal;
                velocity = new_velocity;
            }
            iterations += 1;
        }
        // --- Post-Loop Ground Detection ---
        let mut is_on_ground = false;
        let mut is_on_slope = false;

        // Only check ground if forced OR if we aren't moving upwards and are above water
        if force_ground_test || (actor_fd.inertia.y <= 0.0 && new_position.y >= swim_level) {
            let from = new_position;
            let drop_distance = (2.0 * constants::GROUND_OFFSET)
                + if actor_fd.is_on_ground {
                    constants::STEP_SIZE_DOWN
                } else {
                    0.0
                };
            let to = new_position - Vec3::new(0.0, drop_distance, 0.0);

            // Trace downward to find the floor
            actor_tracer.do_trace(
                collider_link.0,
                from,
                to,
                &spatial_query,
                &colliders_q,
                &collision_layers_q,
            );

            if actor_tracer.fraction < 1.0 {
                let hit_actor = actor_tracer
                    .hit_entity
                    .map_or(false, |e| actor_q.contains(e));

                if !hit_actor {
                    is_on_ground = true;
                    is_on_slope = !is_walkable_slope(actor_tracer.plane_normal);
                    actor_fd.standing_on = actor_tracer.hit_entity;

                    // Check for Water Walking (Assumes your collision layers distinguish water)
                    // if hit_layer == CollisionLayer::Water { actor_fd.walking_on_water = true; }

                    if !actor_fd.flying && !is_on_slope {
                        // If we are a bit above the floor, snap to it + offset
                        if actor_tracer.fraction * drop_distance > constants::GROUND_OFFSET {
                            new_position.y = actor_tracer.end_pos.y + constants::GROUND_OFFSET;
                        } else {
                            // Very close to ground: center the actor to prevent jitter
                            new_position.y = actor_tracer.end_pos.y;
                            actor_tracer.do_trace(
                                collider_link.0,
                                new_position,
                                new_position + Vec3::new(0.0, 2.0 * constants::GROUND_OFFSET, 0.0),
                                &spatial_query,
                                &colliders_q,
                                &collision_layers_q,
                            );
                            new_position = (new_position + actor_tracer.end_pos) / 2.0;
                        }
                    }
                } else {
                    // Vanilla Logic: Actors can stand on each other but it doesn't count as "On Ground"
                    // for the sake of jumping/etc, but prevents them from falling through each other.
                    if !actor_fd.flying
                        && is_walkable_slope(actor_tracer.plane_normal)
                        && (actor_tracer.end_pos.y + constants::GROUND_OFFSET <= new_position.y)
                    {
                        new_position.y = actor_tracer.end_pos.y + constants::GROUND_OFFSET;
                    }
                    is_on_ground = false;
                }
            }

            // Stuck logic: If the engine flagged us as stuck, treat us as on ground to escape the state
            if actor_fd.stuck_frames > 0 {
                is_on_ground = true;
                is_on_slope = false;
            }
        }

        // --- Gravity and Inertia Application ---
        if (is_on_ground && !is_on_slope) || new_position.y < swim_level || actor_fd.flying {
            // Reset inertia if standing still, swimming, or flying
            actor_fd.inertia = Vec3::ZERO;
        } else {
            // Apply Gravity (Y-down)
            // Morrowind uses: gravity * units_per_meter (roughly 9.8 * 21.95)
            actor_fd.inertia.y -= delta_time * constants::GRAVITY_CONST;

            // Apply SlowFall (Levitation/Spells)
            if actor_fd.inertia.y < 0.0 {
                actor_fd.inertia.y *= actor_fd.slow_fall_mult;
            }

            // SlowFall also affects horizontal air momentum in Morrowind
            if actor_fd.slow_fall_mult < 1.0 {
                actor_fd.inertia.x *= actor_fd.slow_fall_mult;
                actor_fd.inertia.z *= actor_fd.slow_fall_mult;
            }
        }

        // --- Finalize Position ---
        actor_fd.is_on_ground = is_on_ground;
        actor_fd.is_on_slope = is_on_slope;
        actor_fd.position = new_position;
        position_interpolation.velocity = velocity;
    }
}

pub fn sync_actor_physics_system(
    actor_q: Query<(
        &ActorFrameData,
        &mut ActorPhysics,
        &mut PositionInterpolation,
        &mut CreatureStats,
        Option<&mut Player>,
    )>,
) {
    for (
        actor_fd,
        mut actor_physics,
        mut position_interpolation,
        mut creature_stats,
        mut player_opt,
    ) in actor_q
    {
        let height_diff = actor_fd.position.y - actor_fd.old_height;
        let is_still_on_ground = actor_fd.was_on_ground && actor_fd.is_on_ground;
        let mut standing_on = None;
        if is_still_on_ground
            || actor_fd.flying
            || actor_fd.position.y < actor_fd.swim_level
            || actor_fd.slow_fall_mult < 1.0
        {
            // Tell the creature to land, which checks/updates fields if it's the player
            creature_stats.land(player_opt.as_deref_mut());
        } else if height_diff < 0.0 {
            creature_stats.fall_height -= height_diff;
        }
        actor_physics.last_stuck_position = actor_fd.last_stuck_position;
        actor_physics.stuck_frames = actor_fd.stuck_frames;
        if let Some(entity) = actor_fd.standing_on {
            standing_on = Some(entity);
        }
        actor_physics.standing_on = standing_on;
        if actor_physics.on_ground == actor_fd.was_on_ground {
            actor_physics.on_ground = actor_fd.is_on_ground;
        }
        actor_physics.on_slope = actor_fd.is_on_slope;
        actor_physics.walking_on_water = actor_fd.walking_on_water;
        actor_physics.force = actor_fd.inertia;
        position_interpolation.current_position = actor_fd.position;
    }
}
