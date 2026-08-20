use avian3d::math::{Dir, Quaternion, Scalar, Vector3};
use avian3d::prelude::*;
use bevy::prelude::*;

use crate::physics::collision::MWCollisionGroup;

pub struct SweepHit {
    pub entity: Entity,
    pub normal: Vector3,
    pub point: Vector3,
    pub fraction: Scalar,
}

pub fn sweep_helper(
    spatial_query: &SpatialQuery,
    colliders: &Query<(&Transform, &Collider)>,
    collision_layers: &Query<&CollisionLayers>,
    collider_entity: Entity,
    from: Vector3,
    to: Vector3,
) -> Option<SweepHit> {
    let sweep_vector = to - from;
    let max_distance = sweep_vector.length();

    let Ok(direction) = Dir::new(sweep_vector) else {
        return None;
    };

    let (collider_transform, collider) = colliders.get(collider_entity).ok()?;

    let config = ShapeCastConfig {
        max_distance,
        target_distance: 0.0,
        compute_contact_on_penetration: true,
        ignore_origin_penetration: true,
    };

    // Standard filter for the sweep
    let filter = SpatialQueryFilter::from_excluded_entities([collider_entity]);
    let mut rejected = Vec::new();

    loop {
        let mut current_filter = filter.clone();
        if !rejected.is_empty() {
            current_filter = current_filter.with_excluded_entities(rejected.clone());
        }

        // Sweep the collider along from->to
        let cast = spatial_query.cast_shape(
            collider,
            from,
            collider_transform.rotation,
            direction,
            &config,
            &current_filter,
        );

        let hit = cast?;
        let hit_layers = collision_layers.get(hit.entity).ok()?;
        let hit_membership = hit_layers.memberships.0;

        // --- Case 1: Actor-Actor overlap ---
        if (hit_membership & MWCollisionGroup::ACTOR.bits()) != 0 {
            // Checks if hit.entity is currently intersecting with our actor at the origin of the
            // shapecast
            let intersections = spatial_query.shape_intersections(
                collider,
                from,
                Quaternion::IDENTITY,
                &SpatialQueryFilter::from_excluded_entities([collider_entity]),
            );

            if intersections.contains(&hit.entity) {
                let mut normal = from - hit.point1;
                normal.z = 0.0;

                if let Some(normal_normalized) = normal.try_normalize() {
                    // Check if moving towards hit actor
                    // dot < 0 means heading toward the surface normal
                    if normal_normalized.dot(*direction) < 0.0 {
                        return Some(SweepHit {
                            entity: hit.entity,
                            normal: normal_normalized,
                            point: hit.point1,
                            fraction: 0.0, // Force instant collision
                        });
                    }
                }
                // Overlapping but moving away: ignore this specific hit and keep sweeping
                rejected.push(hit.entity);
                continue;
            }
        }

        // --- Case 2: Projectile ---
        if (hit_membership & MWCollisionGroup::PROJECTILE.bits()) != 0 {
            // TODO:: if projectile.is_valid_target(me) projectile.hit(me)
            rejected.push(hit.entity);
            continue;
        }

        // --- Case 3: Environment collisions ---
        let dot_collision = direction.dot(hit.normal1);

        // we ignore if dot >= 0.0 (moving parallel or away)
        if dot_collision >= 0.0 {
            rejected.push(hit.entity);
            continue;
        }

        return Some(SweepHit {
            entity: hit.entity,
            normal: hit.normal1,
            point: hit.point1,
            fraction: if max_distance > 0.0 {
                hit.distance / max_distance
            } else {
                0.0
            },
        });
    }
}
