// In src/client/physics/unstuck.rs

use bevy::prelude::*;
use bevy_rapier3d::{
    na::RealField,
    parry::query::contact as parry_contact,
    prelude::{Collider, QueryFilter, RapierContext},
    rapier::prelude::Isometry,
};

use tes3mp_rust::actor_defs::ActorState;

use super::{
    CreaturePhysicsState,
    constants::{ALLOWED_PENETRATION, COLLISION_MARGIN, FAILSAFE_UNSTUCK_UPWARD_DISTANCE},
};

#[derive(Debug, Default, Clone, Copy)]
struct UnstuckContactData {
    deepest_penetration: f32,
    contact_sum: Vec3,
    max_x: f32,
    max_y: f32,
    max_z: f32,
}

/// Queries a hypothetical position for penetrations using direct, stateless geometry queries.
fn query_potential_position(
    context: &RapierContext,
    collider_query: &Query<(&GlobalTransform, &Collider, Option<&ActorState>)>,
    root_entity: Entity,
    controller: &CreaturePhysicsState,
    test_pos: Vec3,
    test_rot: Quat,
) -> UnstuckContactData {
    let mut contact_data = UnstuckContactData::default();
    let character_shape = &controller.collision_shape;
    let character_iso: Isometry<f32> = (test_pos, test_rot).into();

    // Broad-phase: Find all shapes we are potentially overlapping with.
    context.intersections_with_shape(
        test_pos,
        test_rot,
        character_shape,
        QueryFilter::new().exclude_rigid_body(root_entity),
        |hit_entity| {
            // Narrow-phase: For each potential overlap, get its shape and transform.
            if let Ok((hit_transform, hit_collider, _)) = collider_query.get(hit_entity) {
                let (_scale, rot, trans) = hit_transform.to_scale_rotation_translation();
                let hit_iso: Isometry<f32> = (trans, rot).into();

                // Perform the direct, stateless contact query using the Parry library function.
                if let Ok(Some(contact)) = parry_contact(
                    &character_iso,
                    character_shape.raw.as_ref(),
                    &hit_iso,
                    hit_collider.raw.as_ref(),
                    0.0, // Prediction distance. 0.0 checks for actual penetration.
                ) {
                    let normal = Vec3::from(contact.normal1); // Use the normal on the object we hit
                    let dist = contact.dist;
                    if dist < 0.0 {
                        // Negative distance is penetration.

                        // Optimization: If we are penetrating but moving away from the surface,
                        // ignore this contact. This can prevent fighting with the depenetration logic
                        if normal.dot(controller.velocity_for_frame) > 0.0 {
                            return true;
                        }
                        let delta = normal * -dist;
                        contact_data.contact_sum += delta;
                        contact_data.max_x = contact_data.max_x.max(delta.x.abs());
                        contact_data.max_y = contact_data.max_y.max(delta.y.abs());
                        contact_data.max_z = contact_data.max_z.max(delta.z.abs());
                        if dist < contact_data.deepest_penetration {
                            contact_data.deepest_penetration = dist;
                        }
                    }
                }
            }
            true // Continue checking for other intersections.
        },
    );

    contact_data
}

pub fn unstuck_character(
    context: &RapierContext,
    collider_query: &Query<(&GlobalTransform, &Collider, Option<&ActorState>)>,
    root_entity: Entity,
    collider_entity: Entity,
    transform: &mut Transform,
    controller: &mut CreaturePhysicsState,
) {
    // Guard clauses remain the same.
    if controller.velocity_for_frame.length_squared() == 0.0 {
        return;
    }
    if controller.stuck_frames >= 10 {
        if (transform.translation - controller.last_stuck_position).length_squared() < 1.0 {
            return;
        } else {
            controller.stuck_frames = 0;
        }
    }

    // --- Stateful Initial Penetration Check ---
    let mut deepest_penetration = 0.0;
    let mut contact_sum = Vec3::ZERO;
    let mut max_x = 0.0;
    let mut max_y = 0.0;
    let mut max_z = 0.0;
    let mut contacts_found = 0;

    // Use `contact_pairs_with` with the now-provided `collider_entity`.
    for contact_pair in context.contact_pairs_with(collider_entity) {
        for manifold in contact_pair.manifolds() {
            for contact_point in manifold.points() {
                // `dist` is negative if there is a penetration.
                if contact_point.dist() < deepest_penetration {
                    deepest_penetration = contact_point.dist();
                }

                let normal = if contact_pair.collider1() == Some(collider_entity) {
                    -manifold.normal()
                } else {
                    manifold.normal()
                };

                let delta = normal * -contact_point.dist();
                contact_sum += delta;
                max_x = max_x.max(delta.x.abs());
                max_y = max_y.max(delta.y.abs());
                max_z = max_z.max(delta.z.abs());
                contacts_found += 1;
            }
        }
    }

    if contacts_found > 0 {
        contact_sum /= contacts_found as f32;
    }

    if deepest_penetration >= -ALLOWED_PENETRATION {
        controller.stuck_frames = 0;
        return;
    }

    // --- The rest of the unstuck logic remains the same ---
    controller.stuck_frames += 1;
    controller.last_stuck_position = transform.translation;

    let collision_shape_offset = Vec3::Y * controller.vertical_offset;
    let initial_shape_position = transform.translation + collision_shape_offset;
    let mut final_shape_position = initial_shape_position;

    let mut ejection_delta = contact_sum;
    if ejection_delta.x.abs() > max_x {
        ejection_delta *= max_x / ejection_delta.x.abs();
    }
    if ejection_delta.y.abs() > max_y {
        ejection_delta *= max_y / ejection_delta.y.abs();
    }
    if ejection_delta.z.abs() > max_z {
        ejection_delta *= max_z / ejection_delta.z.abs();
    }

    // --- Attempt 1: Ejection via Clamped Summed Deltas ---
    let test_pos_1 = initial_shape_position + add_margin_to_delta(ejection_delta);
    let contacts_after_attempt_1 = query_potential_position(
        context,
        collider_query,
        root_entity,
        controller,
        test_pos_1,
        transform.rotation,
    );

    if contacts_after_attempt_1.deepest_penetration > deepest_penetration {
        final_shape_position = test_pos_1;
    } else {
        // --- Attempt 2 ---
        let upward_ejection = Vec3::Y * ejection_delta.y.abs();
        let test_pos_2 = initial_shape_position + add_margin_to_delta(upward_ejection);
        let contacts_after_attempt_2 = query_potential_position(
            context,
            collider_query,
            root_entity,
            controller,
            test_pos_2,
            transform.rotation,
        );
        if contacts_after_attempt_2.deepest_penetration > deepest_penetration {
            final_shape_position = test_pos_2;
        } else {
            // --- Attempt 3 ---
            let fixed_upward_ejection = Vec3::Y * FAILSAFE_UNSTUCK_UPWARD_DISTANCE;
            let test_pos_3 = initial_shape_position + add_margin_to_delta(fixed_upward_ejection);
            let contacts_after_attempt_3 = query_potential_position(
                context,
                collider_query,
                root_entity,
                controller,
                test_pos_3,
                transform.rotation,
            );
            if contacts_after_attempt_3.deepest_penetration > deepest_penetration {
                final_shape_position = test_pos_3;
            }
        }
    }

    transform.translation = final_shape_position - collision_shape_offset;
}
fn add_margin_to_delta(delta: Vec3) -> Vec3 {
    if delta.length_squared() == 0.0 {
        //
        return delta;
    }
    delta + delta.normalize_or_zero() * COLLISION_MARGIN
}
