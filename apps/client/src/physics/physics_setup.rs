use super::CreaturePhysicsState;
use crate::const_defs::NIF_SCALE_MULTIPLIER;
use crate::defs::ColliderEntityLink;
use crate::physics::CreatureControllerLink;
use avian3d::parry::math::Pose3;
use avian3d::prelude::{Collider, CollisionLayers, SleepingDisabled};
use bevy::prelude::*;
use bevy_nif::{BoundData, NeedsNifPhysics, NiType};
use bevy_nif::{loader::Nif, spawner::NifScene};

/// This system runs once for each newly spawned `NifScene` entity which
/// also has a NeedsNifPhysic component.
/// It creates and attaches the appropriate Rapier physics components, handling
/// local collider offsets by spawning a dedicated child entity for the collider.
pub fn setup_nif_physics(
    mut commands: Commands,
    nif_assets: Res<Assets<Nif>>,
    query: Query<(Entity, &NifScene, &NeedsNifPhysics, &CreatureControllerLink)>,
) {
    for (top_level_entity, nif_scene, needs_physics, creature_controller_link) in query.iter() {
        let Some(nif_asset) = nif_assets.get(&nif_scene.0) else {
            continue;
        };
        // Mark the top-level entity so we don't process it again.
        commands
            .entity(top_level_entity)
            .remove::<NeedsNifPhysics>();
        let mut bounding_volumes = Vec::new();
        dbg!(&needs_physics.0);
        for (entity, key) in &needs_physics.0 {
            dbg!("entity {}", entity);
            if let Some(ni_type) = nif_asset.objects.get(*key) {
                match ni_type {
                    NiType::NiNode(ni_node) => {
                        // Guaranteed to have one otherwise it wouldn't be in this list
                        let bounding_volume = ni_node.bounding_volume.as_ref().unwrap().clone();
                        bounding_volumes.push(bounding_volume);
                    }
                    _ => {
                        dbg!(ni_type);
                    }
                }
            }
        }
        // TEMPORARY
        let mut compound_parts = Vec::new();
        for bounding_volume in bounding_volumes {
            match bounding_volume.bound_data {
                BoundData::NiBoxBV(box_bv) => {
                    let bevy_center = Vec3::new(-box_bv.center.x, box_bv.center.z, box_bv.center.y);
                    let mut bevy_extents = Vec3::new(
                        box_bv.extents.x.abs(),
                        box_bv.extents.z.abs(),
                        box_bv.extents.y.abs(),
                    );

                    let scale = NIF_SCALE_MULTIPLIER;
                    //multiplied by 2 for half extents
                    bevy_extents *= 2.0;
                    compound_parts.push((
                        bevy_center * scale,
                        Quat::IDENTITY,
                        Collider::cuboid(
                            bevy_extents.x * scale,
                            bevy_extents.y * scale,
                            bevy_extents.z * scale,
                        ),
                    ));
                }
                _ => {
                    //TODO::
                }
            }
        }

        // --- Determine RigidBody Type
        /*let mut compound_parts: Vec<(Vec3, Quat, Collider)> = Vec::new();
         for collision_shape in &nif_asset.collision_shapes {
            collect_shapes_recursively(collision_shape, &mut compound_parts);
        }

        if compound_parts.is_empty() {
            continue;
        }
        */
        // TEMPORARY
        let compound_collider = Collider::compound(compound_parts);

        let collider_entity = commands
            .spawn((
                Name::new("PhysicsCollider"),
                Visibility::default(),
                Transform::default(),
                GlobalTransform::default(),
                SleepingDisabled,
                compound_collider.clone(), // Attach the collider here
                CollisionLayers::default(),
            ))
            .id();
        let aabb = compound_collider.shape().compute_aabb(&Pose3::IDENTITY);
        // This assumes the entity's transform origin is at its feet. The physics collider's
        // origin is its geometric center, so we must offset it vertically by half its
        // height to align its base with the entity's transform.
        //let vertical_offset = aabb.half_extents().y;
        let vertical_offset = aabb.half_extents().y;
        commands.entity(creature_controller_link.0).insert((
            CreaturePhysicsState {
                // Also add a "copy" (sharedshape) of the collider to our state component for easy access
                // in the movement system. This avoids having to query for children.
                collider: compound_collider.clone(),
                vertical_offset,
                on_ground: false,
                ..default()
            },
            ColliderEntityLink(collider_entity),
        ));

        // Parent the collider entity to the actor root ---
        commands
            .entity(creature_controller_link.0)
            .add_child(collider_entity);
    }
}
/*
/// A recursive helper function to flatten nested collision shapes (like Unions)
/// into a single list of colliders and their local transforms.
fn collect_shapes_recursively(
    nif_shape: &NifCollisionShape,
    parts: &mut Vec<(Vec3, Quat, Collider)>,
) {
    let scale = NIF_SCALE_MULTIPLIER;

    match nif_shape {
        NifCollisionShape::Box { center, extents } => {
            parts.push((
                *center * scale,
                Quat::IDENTITY,
                Collider::cuboid(extents.x * scale, extents.y * scale, extents.z * scale),
            ));
        }
        NifCollisionShape::Capsule {
            extent,
            radius,
            center,
            ..
        } => {
            // The NIF "Capsule" is treated as a cylinder, as in OpenMW
            // Rapier's cylinder function takes the half-height.
            let half_height = (*extent / 2.0) * scale;
            parts.push((
                *center * scale,
                Quat::IDENTITY,
                Collider::cylinder(half_height, *radius * scale),
            ));
        }
        NifCollisionShape::Sphere { center, radius } => {
            parts.push((
                *center * scale,
                Quat::IDENTITY,
                Collider::ball(*radius * scale),
            ));
        }
        NifCollisionShape::Union(shapes) => {
            for sub_shape in shapes {
                collect_shapes_recursively(sub_shape, parts);
            }
        }
        NifCollisionShape::TriangleMesh { .. } => {
            warn!("TriangleMesh in character collision is not recommended.");
        }
    }
}*/
