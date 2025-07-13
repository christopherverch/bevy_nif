use bevy::prelude::*;
use bevy_nif::nif::{loader::Nif, physics::NifCollisionShape, spawner::NifScene};
use bevy_rapier3d::{parry::math::Isometry, prelude::*};

use cdda_bevy::client::const_defs::NIF_SCALE_MULTIPLIER;

use super::CreaturePhysicsState;

#[derive(Component)]
pub struct NeedsNifPhysics(pub Entity);
/// This system runs once for each newly spawned `NifScene` entity which
/// also has a NeedsNifPhysic component.
/// It creates and attaches the appropriate Rapier physics components, handling
/// local collider offsets by spawning a dedicated child entity for the collider.
pub fn setup_nif_physics(
    mut commands: Commands,
    nif_assets: Res<Assets<Nif>>,
    query: Query<(Entity, &NifScene, &NeedsNifPhysics)>,
) {
    for (top_level_entity, nif_scene, needs_physics) in query.iter() {
        let Some(nif_asset) = nif_assets.get(&nif_scene.0) else {
            continue;
        };
        // Mark the top-level entity so we don't process it again.
        commands
            .entity(top_level_entity)
            .remove::<NeedsNifPhysics>();

        if nif_asset.collision_shapes.is_empty() {
            continue;
        };

        let actor_root_entity = needs_physics.0;

        commands.entity(actor_root_entity).insert(Name::new("test"));
        // --- Determine RigidBody Type
        let mut compound_parts: Vec<(Vec3, Quat, Collider)> = Vec::new();
        for collision_shape in &nif_asset.collision_shapes {
            collect_shapes_recursively(collision_shape, &mut compound_parts);
        }

        if compound_parts.is_empty() {
            continue;
        }
        let compound_collider = Collider::compound(compound_parts);
        let collider_entity = commands
            .spawn((
                Name::new("PhysicsCollider"),
                Transform::default(),
                GlobalTransform::default(),
                Sleeping::disabled(),
                compound_collider.clone(), // Attach the collider here
                CollisionGroups::new(Group::GROUP_1, Group::ALL),
                ActiveEvents::COLLISION_EVENTS | ActiveEvents::CONTACT_FORCE_EVENTS,
            ))
            .id();
        let aabb = compound_collider.raw.compute_aabb(&Isometry::identity());
        let vertical_offset = aabb.half_extents().y;
        commands.entity(actor_root_entity).insert(
            // Also add a copy of the collider to our state component for easy access
            // in the movement system. This avoids having to query for children.
            CreaturePhysicsState {
                collision_shape: compound_collider,
                vertical_offset,
                on_ground: false,
                ..default()
            },
        );

        // Parent the collider entity to the actor root ---
        commands
            .entity(actor_root_entity)
            .add_child(collider_entity);
    }
}

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
}
