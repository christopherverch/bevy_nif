use avian3d::prelude::{Collider, RigidBody};
use bevy_ecs::{
    entity::Entity,
    lifecycle::Add,
    observer::On,
    system::{Commands, Query},
};
use bevy_math::{Quat, Vec3};
use nif::{BoundData, NiKey, NiType, loader::Nif};

use crate::{NifColliderData, loader_spawners::NifBox};

pub fn replace_temp_colliders(
    ev: On<Add<NifColliderData>>,
    mut temp_nif_collider_q: Query<(Entity, &mut NifColliderData)>,
    mut commands: Commands,
) {
    let (entity, mut temp_nif_collider) = temp_nif_collider_q.get_mut(ev.entity).unwrap();
    match &mut *temp_nif_collider {
        NifColliderData::Trimesh(positions, indices) => {
            commands.entity(ev.entity).insert((
                Collider::try_trimesh(std::mem::take(positions), std::mem::take(indices)).unwrap(),
                RigidBody::Static,
            ));
            commands.entity(entity).remove::<NifColliderData>();
        }
        _ => {}
    }
}

pub fn handle_bvs(bvs: Vec<(Entity, NiKey)>, nif: &Nif) -> Option<NifColliderData> {
    let mut bounding_volumes = Vec::new();
    for (_entity, key) in bvs {
        if let Some(ni_type) = nif.objects.get(key) {
            match ni_type {
                NiType::NiNode(ni_node) => {
                    // Guaranteed to have one otherwise it wouldn't be in this list
                    let bounding_volume = ni_node.bounding_volume.as_ref().unwrap().clone();
                    bounding_volumes.push(bounding_volume);
                }

                _ => {}
            }
        }
    }

    // TEMPORARY
    let mut compound_parts = Vec::new();
    for bounding_volume in bounding_volumes {
        match bounding_volume.bound_data {
            BoundData::NiBoxBV(box_bv) => {
                let bevy_center = Vec3::new(box_bv.center.x, box_bv.center.y, box_bv.center.z);
                let mut bevy_extents = box_bv.extents.abs();

                //multiplied by 2 because they're half extents
                bevy_extents *= 2.0;

                compound_parts.push(NifBox {
                    translation: bevy_center,
                    rotation: Quat::IDENTITY,
                    extents: bevy_extents,
                });
            }
            _ => {
                //TODO::
            }
        }
    }
    if compound_parts.len() > 0 {
        return Some(NifColliderData::CompoundBox(compound_parts));
    }
    None
}
