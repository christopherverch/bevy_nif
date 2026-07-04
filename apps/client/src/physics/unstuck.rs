use crate::{
    defs::{ColliderEntityLink, LocalPlayer},
    physics::{constants::ALLOWED_PENETRATION, helpers::get_pitch_yaw_3d_approx_movement},
};
use avian3d::{
    collision::collider::contact_query,
    prelude::{Collider, SpatialQuery, SpatialQueryFilter},
};
use bevy::prelude::*;
use common::consts::MW_UNITS_PER_METER;
use mwphysics::ActorFrameData;

use super::constants::COLLISION_MARGIN;

pub fn unstuck_character(
    mut character_query: Query<
        (Entity, &Transform, &mut ActorFrameData, &ColliderEntityLink),
        With<LocalPlayer>,
    >,
    spatial_query: SpatialQuery,
    collider_q: Query<(&Transform, &Collider)>,
) {
    for (root_entity, actor_transform, mut actor_fd, collider_link) in character_query.iter_mut() {
        if actor_fd.skip_collision_detection {
            return;
        }
        // Nothing attempted to move, assume our position is correct
        if actor_fd.movement.length_squared() == 0.0 {
            return;
        }
        const DIST_TRESHOLD: f32 = (10.0 / MW_UNITS_PER_METER) * (10.0 / MW_UNITS_PER_METER);
        if actor_fd.stuck_frames >= 10 {
            if (actor_fd.last_stuck_position - actor_transform.translation).length_squared()
                < DIST_TRESHOLD
            {
                return;
            } else {
                actor_fd.stuck_frames = 0;
                actor_fd.last_stuck_position = Vec3::ZERO;
            }
        }

        // use vanilla-accurate collision hull position hack (do same hitbox offset hack as movement solver)
        // if vanilla compatibility didn't matter, the "correct" collision hull position would be
        // physicActor->getScaledMeshTranslation()
        let vertical_half_extent = Vec3::new(0.0, actor_fd.half_extents_y, 0.0);

        // use a 3d approximation of the movement vector to better judge player intent
        let mut velocity = get_pitch_yaw_3d_approx_movement(&actor_fd);
        if !actor_fd.is_on_ground || actor_fd.is_on_slope {
            velocity += actor_fd.inertia;
        }
        // The collision box is offset so it's not centered on the character's feet, so we have to reverse that
        let ref_position = actor_fd.position + vertical_half_extent;
        let mut good_position = ref_position;
        let Ok((collider_transform, collider)) = collider_q.get(collider_link.0) else {
            return;
        };
        let mut query_transform = *collider_transform;
        let mut gather_contacts = |new_offset: Vec3| {
            // Calculate the new origin with margin
            good_position = ref_position + add_margin_to_delta(new_offset);
            query_transform.translation = good_position;

            let mut contact_sum = Vec3::ZERO;
            let mut max_depth: f32 = 0.0;
            let mut max_xyz = Vec3::ZERO;

            // 1. Use the callback to find intersecting entities
            spatial_query.shape_intersections_callback(
                collider,
                query_transform.translation,
                collider_transform.rotation,
                &SpatialQueryFilter::from_excluded_entities([root_entity]),
                |entity| {
                    // 2. For every entity found, perform a low-level contact query
                    // You need to fetch the other collider and its transform from your Bevy Query
                    if let Ok((other_transform, other_collider)) = collider_q.get(entity) {
                        // Use the contact query module you found!
                        if let Ok(Some(contact)) = contact_query::contact(
                            collider,
                            query_transform.translation,
                            query_transform.rotation,
                            other_collider,
                            other_transform.translation,
                            other_transform.rotation,
                            0.0,
                        ) {
                            // 1. Transform the local normal to world space
                            // contact.local_normal1 is relative to the actor's orientation.
                            let world_normal = query_transform.rotation * contact.local_normal1;

                            // 2. Use 'penetration' for depth
                            let depth = contact.penetration;

                            // 3. 1:1 Intent Filter: Only handle collisions we are moving INTO
                            // We use the world_normal because velocity is in world space.
                            if world_normal.dot(velocity) < 0.0 {
                                contact_sum += world_normal * depth;
                                max_depth = max_depth.max(depth);

                                // Track individual components (mMaxX, mMaxY, mMaxZ)
                                max_xyz.x = max_xyz.x.max((world_normal.x * depth).abs());
                                max_xyz.y = max_xyz.y.max((world_normal.y * depth).abs());
                                max_xyz.z = max_xyz.z.max((world_normal.z * depth).abs());
                            }
                        }
                    }
                    true // Continue searching
                },
            );

            (-max_depth, contact_sum, max_xyz)
        };
        // Step 1: Initial Check
        let (dist, mut pos_delta, max_xyz) = gather_contacts(Vec3::ZERO);

        if dist < -ALLOWED_PENETRATION {
            actor_fd.stuck_frames += 1;
            actor_fd.last_stuck_position = actor_fd.position;

            // Component Limiting (matches OpenMW's positionDelta scaling)
            if pos_delta.x.abs() > max_xyz.x {
                pos_delta.x *= max_xyz.x / pos_delta.x.abs();
            }
            if pos_delta.y.abs() > max_xyz.y {
                pos_delta.y *= max_xyz.y / pos_delta.y.abs();
            }
            if pos_delta.z.abs() > max_xyz.z {
                pos_delta.z *= max_xyz.z / pos_delta.z.abs();
            }

            // Attempt 1: Full Rejection
            let (dist2, _, _) = gather_contacts(pos_delta);
            if dist2 > dist {
                // Use the good_position that was updated inside the closure call above
                actor_fd.position = good_position - vertical_half_extent;
            } else {
                // Attempt 2: Upwards only
                let upwards = Vec3::new(0.0, pos_delta.y.abs(), 0.0);
                let (dist3, _, _) = gather_contacts(upwards);
                if dist3 > dist {
                    actor_fd.position = good_position - vertical_half_extent;
                } else {
                    // Attempt 3: Fixed distance up
                    let fixed_up = Vec3::new(0.0, 10.0 / MW_UNITS_PER_METER, 0.0);
                    let (dist4, _, _) = gather_contacts(fixed_up);
                    if dist4 > dist {
                        actor_fd.position = good_position - vertical_half_extent;
                    }
                }
            }
        } else {
            actor_fd.stuck_frames = 0;
            actor_fd.last_stuck_position = Vec3::ZERO;
        }
    }
}
fn add_margin_to_delta(delta: Vec3) -> Vec3 {
    if delta.length_squared() == 0.0 {
        //
        return delta;
    }
    delta + delta.normalize_or_zero() * COLLISION_MARGIN
}
