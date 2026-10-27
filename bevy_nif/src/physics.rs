use avian3d::prelude::{Collider, RigidBody};
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    observer::On,
    system::{Commands, Query},
    world::{CommandQueue, World},
};
use bevy_math::{Quat, Vec3};
use bevy_transform::components::{GlobalTransform, Transform};
use bevy_world_serialization::WorldInstanceReady;
use nif::{BoundData, NiKey, NiType, loader::Nif};

use crate::{NifColliderData, loader::TrimeshesWithColliders, loader_spawners::NifBox};
#[derive(Component)]
pub struct ComputeColliderTask(Task<CommandQueue>);

pub fn spawn_collider_tasks(
    _ev: On<WorldInstanceReady>,
    mut trimeshes_with_colliders_q: Query<&mut TrimeshesWithColliders>,
    mut temp_nif_collider_q: Query<(Entity, &mut NifColliderData, &GlobalTransform, &Transform)>,
    mut commands: Commands,
) {
    let thread_pool = AsyncComputeTaskPool::get();

    for mut trimeshes_with_colliders in &mut trimeshes_with_colliders_q {
        for entity in trimeshes_with_colliders.0.drain(..) {
            let Ok((entity, mut temp_nif_collider, global_transform, transform)) =
                temp_nif_collider_q.get_mut(entity)
            else {
                continue;
            };

            // Take ownership of data so we can move it safely into the async task
            let (mut positions, indices) = match &mut *temp_nif_collider {
                NifColliderData::Trimesh(p, i) => (std::mem::take(p), std::mem::take(i)),
                _ => continue,
            };

            let scale = global_transform.scale();
            let translation = transform.translation;

            let task = thread_pool.spawn(async move {
                for position in &mut positions {
                    *position *= scale;
                    *position += translation;
                }

                let mut command_queue = CommandQueue::default();

                if let Ok(collider) = Collider::try_trimesh(positions, indices) {
                    command_queue.push(move |world: &mut World| {
                        world
                            .entity_mut(entity)
                            .insert((collider, RigidBody::Static));
                        world.entity_mut(entity).remove::<NifColliderData>();
                    });
                }

                command_queue
            });

            // Attach the task component to the entity
            commands.entity(entity).insert(ComputeColliderTask(task));
        }
    }
}

pub fn handle_collider_tasks(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut ComputeColliderTask)>,
) {
    for (entity, mut task) in &mut tasks {
        // Non-blocking check if background task is finished
        if let Some(mut command_queue) = check_ready(&mut task.0) {
            commands.append(&mut command_queue);
            commands.entity(entity).remove::<ComputeColliderTask>();
        }
    }
}

pub fn handle_bvs(bvs_opt: Option<Vec<(Entity, NiKey)>>, nif: &Nif) -> Option<NifColliderData> {
    let Some(bvs) = bvs_opt else {
        return None;
    };
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
