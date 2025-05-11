use std::collections::VecDeque;

use crate::NifInstantiated;

use super::animation::BoneMap;
use bevy::{prelude::*, render::render_resource::Face};

#[derive(Component, PartialEq, Clone, Debug)]
pub enum AttachmentType {
    MainSkeleton {
        skeleton_id: u64,
    },
    Skinned {
        skeleton_id: u64,
    },
    Rigid {
        skeleton_id: u64,
        target_bone: String,
    },
    DoubleSidedRigid {
        skeleton_id: u64,
        target_bone: String,
    },
    // Maybe Morphed { target_bone: String }, later
}
impl AttachmentType {
    /// Returns the `target_skeleton_id` if the attachment type is
    /// Skinned, Rigid, or DoubleSidedRigid. Otherwise, returns None.
    pub fn get_target_skeleton_id(&self) -> u64 {
        match self {
            AttachmentType::Skinned { skeleton_id }
            | AttachmentType::Rigid { skeleton_id, .. }
            | AttachmentType::DoubleSidedRigid { skeleton_id, .. }
            | AttachmentType::MainSkeleton { skeleton_id } => *skeleton_id,
        }
    }
}
pub fn attach_parts(
    trigger: Trigger<NifInstantiated>,
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
    //make sure this is a nif that has a target skeleton
    let Some(skeleton_id) = trigger.skeleton_id_opt else {
        return;
    };
    //make sure the target skeleton exists
    if bone_map
        .root_skeleton_entity_map
        .get(&skeleton_id)
        .is_none()
    {
        return;
    }

    for (nifscene_root, attach_type) in attach_query.iter() {
        println!("attach type {:?}", attach_type);
        if let Some(bodypart_mesh) = find_child_of_child_with_name_containing(
            &all_entities_with_children,
            &names,
            &nifscene_root,
            "NifScene",
        ) {
            if let AttachmentType::Rigid {
                skeleton_id,
                target_bone,
            } = attach_type
            {
                let target_bone_ninode = &format!("NiNode: {}", target_bone).to_string();
                if let Some(bone_entities_map) = bone_map.bone_entities_map.get(skeleton_id) {
                    if let Some(skeleton_bone) = bone_entities_map.get(target_bone_ninode) {
                        if target_bone.contains("Left") {
                            println!("target bone: {}", target_bone);
                            //find the child of the ninode(should be trimesh), so we can get the mesh and material of
                            //the trimesh
                            let entities = find_descendants_with_name_containing(
                                &all_entities_with_children,
                                &names,
                                bodypart_mesh,
                                "NiTriShape",
                            );
                            for trishape in entities {
                                if let Ok(mut transform) = transforms.get_mut(trishape) {
                                    transform.translation.x = -transform.translation.x;
                                    transform.rotation.y = -transform.rotation.y;
                                    transform.rotation.z = -transform.rotation.z;
                                    transform.scale.x = -1.0;
                                }
                                if let Ok((mesh3d, material)) = materials_query.get(trishape) {
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
                                        commands.entity(trishape).insert(Mesh3d(mesh_handle));
                                    }
                                    if let Some(standard_material) = materials.get_mut(&material.0)
                                    {
                                        standard_material.cull_mode = Some(Face::Front);
                                        standard_material.double_sided = true;
                                    }
                                }
                            }
                        }
                        commands.entity(nifscene_root).set_parent(*skeleton_bone);
                        commands.entity(nifscene_root).remove::<AttachmentType>();
                    }
                }
            } else {
                match attach_type {
                    //don't parent main skeleton to itself
                    AttachmentType::MainSkeleton { .. } => {}
                    _ => {
                        if let Some(skeleton_root) = bone_map
                            .root_skeleton_entity_map
                            .get(&attach_type.get_target_skeleton_id())
                        {
                            commands.entity(nifscene_root).set_parent(*skeleton_root);
                            commands.entity(nifscene_root).remove::<AttachmentType>();
                        }
                    }
                }
            }
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
pub fn find_descendants_with_name_containing(
    all_entities_with_children: &Query<&Children>,
    names: &Query<&Name>,
    start_entity: Entity,
    name_to_match: &str,
) -> Vec<Entity> {
    let mut found_entities = Vec::new(); // Stores all entities that match the criteria
    let mut queue = VecDeque::new(); // Queue for the BFS

    // Start the search with the initial entity
    queue.push_back(start_entity);

    while let Some(current_entity) = queue.pop_front() {
        // Check if the current entity has a name and if it matches
        if let Ok(name_component) = names.get(current_entity) {
            // Use as_str() for more direct string access from the Name component
            if name_component.as_str().contains(name_to_match) {
                found_entities.push(current_entity); // Add this entity to the results
            }
        }

        // Add all children of the current entity to the queue for further searching
        if let Ok(children) = all_entities_with_children.get(current_entity) {
            for &child_entity in children.iter() {
                // children.iter() yields &Entity
                queue.push_back(child_entity);
            }
        }
    }

    found_entities // Return all entities found
}
