use crate::{
    nif_animation::SkeletonMap,
    spawner::{NifInstantiated, NifNodeIndex},
};

use bevy::{mesh::VertexAttributeValues, prelude::*, render::render_resource::Face};

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
    /// THIS ISN'T USED IN THE ATTACHMENT LOGIC. This is only a marker for users,
    /// and should be changed to Rigid with "Left " being prepended to the target_bone
    DoubleSidedRigid {
        skeleton_id: u64,
        target_bone: String,
    },
    // Maybe Morphed { target_bone: String }, later
}
impl AttachmentType {
    /// Returns the skeleton id the attachment is targetting
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
    event: On<NifInstantiated>,
    attach_query: Query<(Entity, &AttachmentType)>,
    skeleton_map: Res<SkeletonMap>,
    mut commands: Commands,
    mut transforms: Query<&mut Transform>,
    materials_query: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    nif_node_index_q: Query<&NifNodeIndex>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    //make sure this is a nif that has a target skeleton, otherwise nothing to attach to
    let Some(skeleton_id) = event.skeleton_id_opt else {
        return;
    };
    //make sure the target skeleton exists
    if skeleton_map
        .root_skeleton_entity_map
        .get(&skeleton_id)
        .is_none()
    {
        error!("nif tried to attach to skeleton but is missing a skeleton to attach to!");
        return;
    }
    if let Ok((nifscene_root, attach_type)) = attach_query.get(event.entity) {
        let Ok(nif_index) = nif_node_index_q.get(nifscene_root) else {
            error!("NifScene missing NifNodeIndex!");
            return;
        };
        if let AttachmentType::Rigid {
            skeleton_id,
            target_bone,
        } = attach_type
        {
            let Some(skeleton) = skeleton_map.skeletons.get(skeleton_id) else {
                error!("nif tried to attach to skeleton but is missing a skeleton to attach to!");
                return;
            };
            let Some(skeleton_bone) = skeleton.get_bone_by_name(target_bone) else {
                error!("nif tried to attach to nonexistant bone!");
                return;
            };
            // Flip rotations and translations for attachments to Left bones
            if target_bone.contains("Left") {
                //find the child of the ninode(should be trimesh), so we can get the mesh and material of
                //the trimesh

                for trishape in &nif_index.tri_shapes {
                    if let Ok(mut transform) = transforms.get_mut(*trishape) {
                        transform.translation.x = -transform.translation.x;
                        transform.rotation.y = -transform.rotation.y;
                        transform.rotation.z = -transform.rotation.z;
                        transform.scale.x = -1.0;
                    }
                    // TODO:: maybe do the normal flipping in a shader so we don't have to clone
                    // the whole mesh
                    if let Ok((mesh3d, material)) = materials_query.get(*trishape) {
                        if let Some(mesh) = meshes.get_mut(&mesh3d.0) {
                            let mut clone_mesh = mesh.clone();
                            if let Some(VertexAttributeValues::Float32x3(normals)) =
                                clone_mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL)
                            {
                                //Flip normals since we flipped x scale
                                for normal in normals {
                                    normal[0] *= -1.0;
                                    normal[1] *= -1.0;
                                    normal[2] *= -1.0;
                                }
                            }
                            let mesh_handle = meshes.add(clone_mesh);
                            commands.entity(*trishape).insert(Mesh3d(mesh_handle));
                        }
                        if let Some(standard_material) = materials.get_mut(&material.0) {
                            standard_material.cull_mode = Some(Face::Front);
                            standard_material.double_sided = true;
                        }
                    }
                }
            }
            // If it's not a left bone, just add it as a child of the skeleton bone
            commands
                .entity(nifscene_root)
                .insert(ChildOf(skeleton_bone.entity));

            commands.entity(nifscene_root).remove::<AttachmentType>();
        } else {
            match attach_type {
                //don't parent main skeleton to itself
                AttachmentType::MainSkeleton { .. } => {}
                AttachmentType::Skinned { .. } => {
                    // Skinned sets the root as parent
                    if let Some(skeleton_root) = skeleton_map
                        .root_skeleton_entity_map
                        .get(&attach_type.get_target_skeleton_id())
                    {
                        commands
                            .entity(nifscene_root)
                            .insert(ChildOf(*skeleton_root));
                        commands.entity(nifscene_root).remove::<AttachmentType>();
                    }
                }
                _ => { // DoubleSidedRigid is only used as a marker for the user
                }
            }
        }
    }
}
