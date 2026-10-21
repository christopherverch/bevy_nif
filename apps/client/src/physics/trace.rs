use bevy::prelude::*;

use avian3d::{
    math::Real,
    prelude::{Collider, CollisionLayers, SpatialQuery},
};

use crate::physics::actor_sweep::sweep_helper;

pub struct ActorTracer {
    pub end_pos: Vec3,
    pub plane_normal: Vec3,
    pub hit_point: Vec3,
    pub hit_entity: Option<Entity>,
    pub fraction: Real,
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
