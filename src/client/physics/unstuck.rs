// In src/client/physics/unstuck.rs

use crate::physics::{components::CreaturePhysicsState, constants::ALLOWED_PENETRATION};
use bevy::prelude::*;
use bevy_rapier3d::{
    // This is the correct, low-level function we need from the underlying geometry library.
    parry::query::contact as parry_contact,
    prelude::{Collider, QueryFilter, RapierContext},
    rapier::prelude::Isometry,
};
use cdda_bevy::client::const_defs::UNITS_PER_METER;

use super::constants::COLLISION_MARGIN;

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
    collider_query: &Query<(&GlobalTransform, &Collider)>,
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
            if let Ok((hit_transform, hit_collider)) = collider_query.get(hit_entity) {
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
    collider_query: &Query<(&GlobalTransform, &Collider)>,
    root_entity: Entity,
    transform: &mut Transform,
    controller: &mut CreaturePhysicsState,
) {
    if controller.velocity_for_frame.length_squared() == 0.0 {
        //
        return;
    }
    if controller.stuck_frames >= 10 {
        //
        if (transform.translation - controller.last_stuck_position).length_squared() < 1.0 {
            return;
        } else {
            controller.stuck_frames = 0; //
        }
    }

    // --- FIX: Use the vertical offset for all position tests ---
    // This is the "vanilla-accurate" hack from OpenMW.
    let collision_shape_offset = Vec3::Y * controller.vertical_offset;
    let initial_shape_position = transform.translation + collision_shape_offset;

    // Perform the initial query at the collision shape's current position.
    let initial_contacts = query_potential_position(
        context,
        collider_query,
        root_entity,
        controller,
        initial_shape_position,
        transform.rotation,
    );

    if initial_contacts.deepest_penetration >= -ALLOWED_PENETRATION {
        //
        controller.stuck_frames = 0;
        return;
    }
    dbg!("was stuck!");

    controller.stuck_frames += 1;
    controller.last_stuck_position = transform.translation;

    let mut best_position = initial_shape_position;
    let mut best_penetration = initial_contacts.deepest_penetration;

    let mut ejection_delta = initial_contacts.contact_sum;

    // Clamp the ejection delta to the largest single-axis penetration [cite: 320-322].
    if ejection_delta.x.abs() > initial_contacts.max_x {
        ejection_delta *= initial_contacts.max_x / ejection_delta.x.abs();
    }
    if ejection_delta.y.abs() > initial_contacts.max_y {
        ejection_delta *= initial_contacts.max_y / ejection_delta.y.abs();
    }
    if ejection_delta.z.abs() > initial_contacts.max_z {
        ejection_delta *= initial_contacts.max_z / ejection_delta.z.abs();
    }

    // --- Attempt 1: Ejection via Clamped Summed Deltas --- [cite: 323]
    let test_pos_1 = initial_shape_position + add_margin_to_delta(ejection_delta);
    let contacts_after_attempt_1 = query_potential_position(
        context,
        collider_query,
        root_entity,
        controller,
        test_pos_1,
        transform.rotation,
    );

    if contacts_after_attempt_1.deepest_penetration > best_penetration {
        best_penetration = contacts_after_attempt_1.deepest_penetration;
        best_position = test_pos_1;
    }

    // --- Attempt 2 & 3 (Fallback logic) ---
    if best_position == initial_shape_position {
        // --- Attempt 2: Upwards-Only Ejection --- [cite: 324]
        // Use the vertical component of the clamped delta.
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

        if contacts_after_attempt_2.deepest_penetration > best_penetration {
            best_penetration = contacts_after_attempt_2.deepest_penetration;
            best_position = test_pos_2;
        } else {
            // --- Attempt 3: Fixed-Distance Upwards Ejection --- [cite: 326]
            let fixed_upward_ejection = Vec3::Y * (10.0 / UNITS_PER_METER);
            let test_pos_3 = initial_shape_position + add_margin_to_delta(fixed_upward_ejection);
            let contacts_after_attempt_3 = query_potential_position(
                context,
                collider_query,
                root_entity,
                controller,
                test_pos_3,
                transform.rotation,
            );

            if contacts_after_attempt_3.deepest_penetration > best_penetration {
                best_position = test_pos_3;
            }
        }
    }

    // --- FIX: Apply the best position found, correcting for the offset ---
    // The `best_position` is the new center of our collision shape. We must
    // subtract the offset to get the new root transform position.
    transform.translation = best_position - collision_shape_offset;
}

fn add_margin_to_delta(delta: Vec3) -> Vec3 {
    if delta.length_squared() == 0.0 {
        //
        return delta;
    }
    delta + delta.normalize_or_zero() * COLLISION_MARGIN
}
