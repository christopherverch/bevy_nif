use avian3d::{
    math::{Scalar, Vector3},
    prelude::{Collider, CollisionLayers, SpatialQuery},
};
use bevy_ecs::{entity::Entity, system::Query};
use bevy_transform::components::Transform;
use glam::Vec3;

use crate::actor_sweep::sweep_helper;
pub struct ActorTracer {
    pub end_pos: Vector3,
    pub plane_normal: Vector3,
    pub hit_point: Vector3,
    pub hit_entity: Option<Entity>,
    pub fraction: Scalar,
}

impl Default for ActorTracer {
    fn default() -> Self {
        Self {
            end_pos: Vec3::ZERO,
            plane_normal: Vec3::Y,
            hit_point: Vec3::ZERO,
            hit_entity: None,
            fraction: 1.0,
        }
    }
}
impl ActorTracer {
    pub fn find_ground(
        &mut self,
        collider_entity: Entity,
        start: Vector3,
        end: Vector3,
        spatial_query: &SpatialQuery,
        colliders: &Query<(&Transform, &Collider)>,
        collider_layers: &Query<&CollisionLayers>,
    ) {
        let hit = sweep_helper(
            spatial_query,
            colliders,
            collider_layers,
            collider_entity,
            start,
            end,
        );

        if let Some(trace) = hit {
            self.fraction = trace.fraction;
            self.plane_normal = trace.normal;
            self.hit_point = trace.point;
            self.hit_entity = Some(trace.entity);
            // Resulting position is start + (end - start) * fraction
            self.end_pos = start.lerp(end, trace.fraction);
        } else {
            // No hit: destination reached
            self.end_pos = end;
            self.plane_normal = Vector3::Y;
            self.hit_point = Vector3::ZERO;
            self.hit_entity = None;
            self.fraction = 1.0;
        }
    }
    pub fn do_trace(
        &mut self,
        collider_entity: Entity,
        start: Vec3,
        end: Vec3,
        spatial_query: &SpatialQuery,
        colliders: &Query<(&Transform, &Collider)>,
        collision_layers: &Query<&CollisionLayers>,
    ) {
        let hit = sweep_helper(
            spatial_query,
            colliders,
            collision_layers,
            collider_entity,
            start,
            end,
        );

        if let Some(trace) = hit {
            self.fraction = trace.fraction;
            self.plane_normal = trace.normal;
            self.hit_point = trace.point;
            self.hit_entity = Some(trace.entity);
            // Standard lerp for the final position
            self.end_pos = start.lerp(end, self.fraction);
        } else {
            self.end_pos = end;
            self.plane_normal = Vec3::Y;
            self.fraction = 1.0;
            self.hit_point = end;
            self.hit_entity = None;
        }
    }
}
