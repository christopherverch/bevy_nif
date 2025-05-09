use std::collections::VecDeque;

use crate::NifInstantiated;

use super::animation::BoneMap;
use bevy::prelude::*;

#[derive(Component, Clone)]
pub enum AttachmentType {
    Skinned, // Default?
    Rigid { target_bone: String }, // e.g., Rigid { target_bone: "Bip01 Head".to_string() }
             // Maybe Morphed { target_bone: String }, later
}
pub fn attach_parts(
    _: Trigger<NifInstantiated>,
    all_entities_with_children: Query<&Children>,
    names: Query<&Name>,
    attach_query: Query<(Entity, &AttachmentType)>,
    bone_map: Res<BoneMap>,
    mut commands: Commands,
    mut transforms: Query<&mut Transform>,
    materials_query: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if bone_map.root_skeleton_entity.is_none() {
        return;
    }
    for (entity_root, attach_type) in attach_query.iter() {
        if let AttachmentType::Rigid { target_bone } = attach_type {
            let target_bone_ninode = &format!("NiNode: {}", target_bone).to_string();
            if let Some(skeleton_bone) = bone_map.entity_map.get(target_bone_ninode) {
                if let Some(entity) = find_child_of_child_with_name_containing(
                    &all_entities_with_children,
                    &names,
                    &entity_root,
                    "NifScene",
                ) {
                    if let Ok(mut transform) = transforms.get_mut(entity) {
                        if target_bone.contains("Left")
                            && !target_bone.contains("Leg")
                            && !target_bone.contains("Knee")
                        {
                            transform.translation.x = -transform.translation.x;
                            transform.scale.x = -1.0;
                            let entity = if let Some(entity_child) =
                                find_child_of_child_with_name_containing(
                                    &all_entities_with_children,
                                    &names,
                                    &entity,
                                    "NiNode",
                                ) {
                                entity_child
                            } else {
                                entity
                            };

                            if let Ok((mesh3d, material)) = materials_query.get(entity) {
                                if let Some(mesh) = meshes.get_mut(&mesh3d.0) {
                                    let mut clone_mesh = mesh.clone();
                                    if let Some(
                                        bevy::render::mesh::VertexAttributeValues::Float32x3(
                                            normals,
                                        ),
                                    ) = clone_mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL)
                                    {
                                        for normal in normals {
                                            normal[0] *= -1.0;
                                            normal[1] *= -1.0;
                                            normal[2] *= -1.0;
                                        }
                                    }
                                    let mesh_handle = meshes.add(clone_mesh);
                                    commands.entity(entity).insert(Mesh3d(mesh_handle));
                                }
                                if let Some(standard_material) = materials.get_mut(&material.0) {
                                    standard_material.cull_mode = None;
                                    standard_material.double_sided = true;
                                }
                            }

                            //transform.rotate_local_x(-std::f32::consts::FRAC_PI_2);
                            //transform.rotate_local_y(std::f32::consts::PI);
                        }
                        commands.entity(entity).set_parent(*skeleton_bone);
                        commands.entity(entity_root).despawn_recursive();
                    }
                }
            }
        } else {
            commands.entity(entity_root).remove::<AttachmentType>();
        }
    }
}
pub fn find_child_of_child_with_name_containing(
    all_entities_with_children: &Query<&Children>,
    names: &Query<&Name>,
    entity: &Entity,
    name_to_match: &str,
) -> Option<Entity> {
    let mut queue = VecDeque::new();
    queue.push_back(entity);
    while let Some(curr_entity) = queue.pop_front() {
        let name_result = names.get(*curr_entity);
        if let Ok(name) = name_result {
            if format!("{}", name).contains(name_to_match) {
                // found the named entity
                if let Ok(child_entities) = all_entities_with_children.get(*curr_entity) {
                    return child_entities.first().copied();
                }
            }
        }

        let children_result = all_entities_with_children.get(*curr_entity);
        if let Ok(children) = children_result {
            for child in children {
                queue.push_back(child)
            }
        }
    }

    None
}
fn rotate_transform_around_world_pivot(transform: &mut Transform, pivot: Vec3, rotation: Quat) {
    // Compute transform as a matrix
    let mat = Mat4::from_translation(transform.translation) * Mat4::from_quat(transform.rotation);

    // Build pivoted rotation: T(pivot) * R * T(-pivot)
    let pivot_rotation =
        Mat4::from_translation(pivot) * Mat4::from_quat(rotation) * Mat4::from_translation(-pivot);

    // Apply pivoted rotation to the object's transform
    let result = pivot_rotation * mat;

    // Extract new translation and rotation
    let (new_translation, new_rotation, _scale) = result.to_scale_rotation_translation();
    transform.translation = new_translation;
    transform.rotation = new_rotation;
}
