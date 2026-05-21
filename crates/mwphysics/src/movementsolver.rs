use crate::{
    MWCollisionGroup,
    actor::ActorPhysics,
    const_defs::{SOLVER_GROUND_OFFSET, SOLVER_LARGE_DISTANCE},
    helpers::is_walkable_slope,
    trace::ActorTracer,
};
use avian3d::prelude::{Collider, CollisionLayers, LayerMask, SpatialQuery, SpatialQueryFilter};
use bevy_ecs::{
    entity::{Entity, EntityHashSet},
    system::Query,
};
use bevy_math::Dir3;
use bevy_transform::components::Transform;
use glam::Vec3;
/// Finds the distance to the ground below a specific position for an actor.
/// Returns (Position, Normal)
pub fn trace_down(
    entity: Entity,
    actor: &mut ActorPhysics,
    actor_position: Vec3,
    max_height: f32,
    spatial_query: &SpatialQuery,
    colliders: &Query<(&Transform, &Collider)>,
    collision_layers: &Query<&CollisionLayers>,
) -> Vec3 {
    // 1. Calculate the physics offset
    // This is the difference between the Actor's feet and the center of its Collider
    // We do this because we don't want to accidentally sweep a collider centered in the middle of a
    // character and act like it's centered on their feet
    let offset = actor.get_collision_object_position(actor_position) - actor_position;
    let start = actor_position + offset;
    // not actually down, but we use it as the down vec everywhere by negating
    let down_vec = Vec3::new(0.0, max_height, 0.0);
    let end = start - down_vec;

    // 2. Perform the Actor Sweep (findGround)
    let mut tracer = ActorTracer::default();
    tracer.find_ground(
        entity,
        start,
        end,
        spatial_query,
        colliders,
        collision_layers,
    );

    // If the shape sweep hit nothing at all
    if tracer.fraction >= 1.0 {
        actor.on_ground = false;
        return actor_position;
    }
    actor.on_ground = true;

    // 3. The Ray Test (Sanity Check)
    // Ray starts exactly at the logical position (feet) and goes down
    let ray_from = actor_position;

    // Only collide with the world, not other actors
    let ray_mask = LayerMask((MWCollisionGroup::WORLD | MWCollisionGroup::HEIGHTMAP).bits());

    let max_distance = max_height;
    let direction = Dir3::from_xyz_unchecked(0.0, -1.0, 0.0);

    let ray_result = spatial_query.cast_ray(
        ray_from,
        direction,
        max_distance,
        true, // solid: true means it detects if the ray starts inside a shape
        &SpatialQueryFilter {
            mask: ray_mask,
            excluded_entities: EntityHashSet::default(),
        },
    );

    let final_pos: Vec3;
    let final_normal: Vec3;

    // 4. Compare Ray vs Shape Sweep
    if let Some(ray) = ray_result {
        let ray_hit_point = ray_from + (*direction * ray.distance);
        let dist_sq = (ray_hit_point - (tracer.end_pos - offset)).length_squared();

        // C++: if (ray_hit && (distance_is_large || !is_walkable))
        if dist_sq > SOLVER_LARGE_DISTANCE || !is_walkable_slope(tracer.plane_normal) {
            // Trust the ray instead of the cylinder
            final_pos = ray_hit_point;
            final_normal = ray.normal;

            actor.on_slope = !is_walkable_slope(final_normal);
            return final_pos + Vec3::new(0.0, SOLVER_GROUND_OFFSET, 0.0);
        }
    }

    // 5. Default to the Shape Sweep result
    final_normal = tracer.plane_normal;
    actor.on_slope = !is_walkable_slope(final_normal);

    // Return the logical position + the small hover offset
    (tracer.end_pos - offset) + Vec3::new(0.0, SOLVER_GROUND_OFFSET, 0.0)
}
