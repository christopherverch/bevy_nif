use avian3d::prelude::{Collider, CollisionLayers, SpatialQuery};
use bevy::prelude::*;

use crate::physics::{
    collision::MWCollisionGroup,
    constants::{
        COLLISION_MARGIN, DO_EXTRA_STAIR_HACKS, GROUND_OFFSET, MAX_SLOPE_DEGREES_COS, MIN_STEP,
        MIN_STEP_2, STEP_SIZE_DOWN, STEP_SIZE_UP,
    },
    helpers::{is_walkable_slope, reject},
    trace::ActorTracer,
};

pub struct Stepper<'a, 'w, 's> {
    pub collision_layers_q: &'a Query<'a, 'w, &'s CollisionLayers>,
    pub tracer: ActorTracer,
    pub up_stepper: ActorTracer,
    pub down_stepper: ActorTracer,
}

impl<'a, 'w, 's> Stepper<'a, 'w, 's> {
    pub fn step(
        &mut self,
        collider_entity: Entity,
        position: &mut Vec3,
        velocity: &mut Vec3,
        remaining_time: &mut f32,
        on_ground: bool,
        first_iteration: bool,
        spatial_query: &SpatialQuery,
        collider_q: &Query<(&Transform, &Collider)>,
    ) -> bool {
        // 1. Horizontal movement check
        if velocity.x == 0.0 && velocity.z == 0.0 {
            return false;
        }

        // 2. Trace UP
        // OpenMW: position + osg::Vec3f(0, 0, StepSizeUp)
        let up_target = *position + Vec3::new(0.0, STEP_SIZE_UP, 0.0);
        self.up_stepper.do_trace(
            collider_entity,
            *position,
            up_target,
            spatial_query,
            collider_q,
            self.collision_layers_q,
        );

        let mut up_distance = 0.0;
        if self.up_stepper.hit_entity.is_none() {
            up_distance = STEP_SIZE_UP;
        } else if self.up_stepper.fraction * STEP_SIZE_UP > COLLISION_MARGIN {
            up_distance = self.up_stepper.fraction * STEP_SIZE_UP - COLLISION_MARGIN;
        } else {
            return false; // Ceiled, can't even start the step
        }

        let to_move = *velocity * *remaining_time;
        let tracer_pos = *position + Vec3::new(0.0, up_distance, 0.0);

        let mut tracer_dest = Vec3::ZERO;
        let mut normal_move = to_move;
        let mut move_distance = normal_move.length();
        normal_move = normal_move.normalize_or_zero();

        // 3. Forward & Down Loop (The 3 attempts)
        let mut attempt = 0;
        let mut down_step_size = 0.0;

        while attempt < 3 {
            attempt += 1;

            if attempt == 1 {
                tracer_dest = tracer_pos + to_move;
            } else if !DO_EXTRA_STAIR_HACKS {
                return false;
            } else if attempt == 2 {
                move_distance = MIN_STEP;
                tracer_dest = tracer_pos + normal_move * MIN_STEP;
            } else if attempt == 3 {
                if up_distance > STEP_SIZE_UP {
                    up_distance = STEP_SIZE_UP;
                }
                let tracer_pos = *position + Vec3::new(0.0, up_distance, 0.0);
                move_distance = MIN_STEP_2;
                tracer_dest = tracer_pos + normal_move * MIN_STEP_2;
            }

            // Trace FORWARD
            self.tracer.do_trace(
                collider_entity,
                tracer_pos,
                tracer_dest,
                spatial_query,
                collider_q,
                self.collision_layers_q,
            );

            if self.tracer.hit_entity.is_some() {
                move_distance *= self.tracer.fraction;
                if move_distance <= COLLISION_MARGIN {
                    return false;
                }

                move_distance -= COLLISION_MARGIN;
                tracer_dest = tracer_pos + normal_move * move_distance;

                // Eject from wall margin
                let temp_dest = tracer_dest + self.tracer.plane_normal * COLLISION_MARGIN * 2.0;
                let mut temp_tracer = ActorTracer::default();
                temp_tracer.do_trace(
                    collider_entity,
                    tracer_dest,
                    temp_dest,
                    spatial_query,
                    collider_q,
                    self.collision_layers_q,
                );

                if temp_tracer.fraction > 0.5 {
                    let effective_fraction = temp_tracer.fraction * 2.0 - 1.0;
                    tracer_dest += self.tracer.plane_normal * COLLISION_MARGIN * effective_fraction;
                }
            }

            // 4. Trace DOWN
            if attempt > 2 {
                down_step_size = up_distance;
            } else {
                down_step_size = move_distance + up_distance + STEP_SIZE_DOWN;
            }

            let down_target = tracer_dest + Vec3::new(0.0, -down_step_size, 0.0);
            self.down_stepper.do_trace(
                collider_entity,
                tracer_dest,
                down_target,
                spatial_query,
                collider_q,
                self.collision_layers_q,
            );

            if can_step_down(&self.down_stepper, self.collision_layers_q) {
                break; // Found a valid landing!
            } else {
                // Check if we should try next attempt
                if attempt == 2
                    && move_distance > up_distance - (self.down_stepper.fraction * down_step_size)
                {
                    return false;
                }
                if first_iteration && attempt < 3 {
                    continue;
                }
                return false;
            }
        }

        // 5. Finalize the Step
        let mut down_distance = 0.0;
        if self.down_stepper.fraction * down_step_size > COLLISION_MARGIN {
            down_distance = self.down_stepper.fraction * down_step_size - COLLISION_MARGIN;
        }

        // Prevent falling further than we stepped up if not on ground
        if down_distance - COLLISION_MARGIN - GROUND_OFFSET > up_distance && !on_ground {
            return false;
        }

        let new_pos = tracer_dest + Vec3::new(0.0, -down_distance, 0.0);

        // Don't move if the change is negligible
        if position.distance_squared(new_pos) < COLLISION_MARGIN.powi(2) {
            return false;
        }

        // Velocity adjustment based on the wall we hit during the "Forward" phase
        if self.tracer.hit_entity.is_some() {
            let mut plane_normal = self.tracer.plane_normal;
            if on_ground && !is_walkable_slope(plane_normal) && plane_normal.y != 0.0 {
                plane_normal.y = 0.0;
                plane_normal = plane_normal.normalize_or_zero();
            }
            *velocity = reject(*velocity, plane_normal);
        }

        // Final velocity adjustment for the floor we landed on
        *velocity = reject(*velocity, self.down_stepper.plane_normal);

        *position = new_pos;
        *remaining_time *= 1.0 - self.tracer.fraction;

        true
    }
}
fn can_step_down(stepper: &ActorTracer, collision_layers_q: &Query<&CollisionLayers>) -> bool {
    // Did we hit something?
    let Some(hit_entity) = stepper.hit_entity else {
        return false;
    };
    // Is it a walkable slope?
    if stepper.plane_normal.y <= MAX_SLOPE_DEGREES_COS {
        return false;
    }
    // Is it NOT an actor?
    match collision_layers_q.get(hit_entity) {
        Ok(layers) => !layers.memberships.has_all(MWCollisionGroup::ACTOR.bits()),
        Err(_) => false, // If it has no layers, we can't safely step down on it
    }
}
