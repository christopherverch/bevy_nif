use crate::{
    NifInstanceReady,
    loader::{NeedsSkeletonMapping, NifNodeIndex, PendingNifSkins},
    loader_spawners::PendingNifSkin,
    skeleton::{Skeleton, SkeletonMap},
};

use bevy_asset::Assets;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    observer::On,
    query::With,
    system::{Commands, Query, Res, ResMut},
};
use bevy_log::{error, warn};
use bevy_math::{Mat3, Quat};
use bevy_mesh::{Mesh, Mesh3d, VertexAttributeValues, skinning::SkinnedMesh};
use bevy_pbr::{MeshMaterial3d, StandardMaterial};
use bevy_render::render_resource::Face;
use bevy_transform::components::Transform;
use bevy_world_serialization::WorldInstanceReady;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Component, PartialEq, Clone, Debug)]
pub enum AttachmentType {
    Skinned {
        skeleton_entity: Entity,
    },
    Rigid {
        skeleton_entity: Entity,
        target_bone: String,
    },
    /// THIS ISN'T USED IN THE ATTACHMENT LOGIC. This is only a marker for users,
    /// and should be changed to Rigid with "Left " or "Right " being prepended to the target_bone
    DoubleSidedRigid {
        skeleton_entity: Entity,
        target_bone: String,
    },
    // Maybe Morphed { target_bone: String }, later
}

impl AttachmentType {
    pub fn get_target_skeleton_entity(&self) -> Entity {
        match self {
            AttachmentType::Skinned { skeleton_entity } => *skeleton_entity,
            AttachmentType::Rigid {
                skeleton_entity, ..
            } => *skeleton_entity,
            AttachmentType::DoubleSidedRigid {
                skeleton_entity, ..
            } => *skeleton_entity,
        }
    }
}

pub fn insert_into_skeleton_map(
    event: On<WorldInstanceReady>,
    skeletons_need_setup: Query<(Entity, &mut Skeleton, &ChildOf), With<NeedsSkeletonMapping>>,
    mut skeleton_map: ResMut<SkeletonMap>,
    attach_query: Query<(Entity, &AttachmentType, &Children)>,
    nif_node_index_q: Query<&NifNodeIndex>,
    mut transforms: Query<&mut Transform>,
    materials_query: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    pending_nif_skins_q: Query<(&PendingNifSkins, &ChildOf)>,
    mut pending_nif_skin_q: Query<&mut PendingNifSkin>,
    mut commands: Commands,
) {
    for (skeleton_entity, mut skeleton, child_of) in skeletons_need_setup {
        if child_of.0 == event.entity {
            let owned_skeleton = Skeleton {
                bones: std::mem::take(&mut skeleton.bones),
                name_to_id: std::mem::take(&mut skeleton.name_to_id),
                roots: std::mem::take(&mut skeleton.roots),
            };
            skeleton_map
                .entity_to_skeleton
                .insert(event.entity, owned_skeleton);
            commands
                .entity(skeleton_entity)
                .remove::<NeedsSkeletonMapping>();
            commands.trigger(NifInstanceReady(event.entity));

            for (nifscene_root, attach_type, children) in attach_query {
                attach_parts_common(
                    nifscene_root,
                    attach_type,
                    children,
                    &skeleton_map,
                    &nif_node_index_q,
                    &mut transforms,
                    &materials_query,
                    &mut materials,
                    &mut meshes,
                    &pending_nif_skins_q,
                    &mut pending_nif_skin_q,
                    &mut commands,
                );
            }
            let Ok((nifscene_root, attach_type, children)) = attach_query.get(event.entity) else {
                return;
            };
            attach_parts_common(
                nifscene_root,
                attach_type,
                children,
                &skeleton_map,
                &nif_node_index_q,
                &mut transforms,
                &materials_query,
                &mut materials,
                &mut meshes,
                &pending_nif_skins_q,
                &mut pending_nif_skin_q,
                &mut commands,
            );

            return;
        }
    }
}
pub fn attach_parts_obs(
    ready: On<WorldInstanceReady>,
    skeleton_map: Res<SkeletonMap>,
    attach_query: Query<(Entity, &AttachmentType, &Children)>,
    nif_node_index_q: Query<&NifNodeIndex>,
    mut transforms: Query<&mut Transform>,
    materials_query: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    pending_nif_skins_q: Query<(&PendingNifSkins, &ChildOf)>,
    mut pending_nif_skin_q: Query<&mut PendingNifSkin>,
    mut commands: Commands,
) {
    let Ok((nifscene_root, attach_type, children)) = attach_query.get(ready.entity) else {
        return;
    };
    dbg!(attach_type);
    attach_parts_common(
        nifscene_root,
        attach_type,
        children,
        &skeleton_map,
        &nif_node_index_q,
        &mut transforms,
        &materials_query,
        &mut materials,
        &mut meshes,
        &pending_nif_skins_q,
        &mut pending_nif_skin_q,
        &mut commands,
    );
}
fn attach_parts_common(
    nifscene_root: Entity,
    attach_type: &AttachmentType,
    children: &Children,
    skeleton_map: &SkeletonMap,
    nif_node_index_q: &Query<&NifNodeIndex>,
    transforms: &mut Query<&mut Transform>,
    materials_query: &Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>,
    materials: &mut Assets<StandardMaterial>,
    meshes: &mut Assets<Mesh>,
    pending_nif_skins_q: &Query<(&PendingNifSkins, &ChildOf)>,
    pending_nif_skin_q: &mut Query<&mut PendingNifSkin>,
    commands: &mut Commands,
) {
    if let AttachmentType::Rigid {
        skeleton_entity,
        target_bone,
    } = attach_type
    {
        let Some(skeleton) = skeleton_map.entity_to_skeleton.get(skeleton_entity) else {
            return;
        };
        if target_bone.contains("Left") {
            for child in children {
                let Ok(nif_index) = nif_node_index_q.get(*child) else {
                    continue;
                };

                for trishape in &nif_index.tri_shapes {
                    if let Ok(mut transform) = transforms.get_mut(*trishape) {
                        // 1. Mirror translation and scale
                        transform.translation.x = -transform.translation.x;
                        transform.scale.x = -1.0;

                        // 2. Mirror the rotation matrix across the X axis
                        let mut mat = Mat3::from_quat(transform.rotation);
                        mat.y_axis.x = -mat.y_axis.x;
                        mat.z_axis.x = -mat.z_axis.x;
                        mat.x_axis.y = -mat.x_axis.y;
                        mat.x_axis.z = -mat.x_axis.z;
                        transform.rotation = Quat::from_mat3(&mat);
                    }
                    if let Ok((_, material)) = materials_query.get(*trishape) {
                        if let Some(standard_material) = materials.get(&material.0) {
                            let mut cloned_material = standard_material.clone();
                            cloned_material.cull_mode = Some(Face::Front);
                            commands
                                .entity(*trishape)
                                .insert(MeshMaterial3d(materials.add(cloned_material)));
                        }
                    }
                }
            }
        }

        let Some(skeleton_bone) = skeleton.get_bone_by_name(target_bone) else {
            error!("nif tried to attach to nonexistant bone!");
            return;
        };
        commands
            .entity(nifscene_root)
            .insert(ChildOf(skeleton_bone.entity));

        commands.entity(nifscene_root).remove::<AttachmentType>();
    } else {
        match attach_type {
            AttachmentType::Skinned { skeleton_entity } => {
                for (pending_skins, childof) in pending_nif_skins_q.iter() {
                    if childof.0 != nifscene_root {
                        continue;
                    }
                    let Some(skeleton) = skeleton_map.entity_to_skeleton.get(skeleton_entity)
                    else {
                        dbg!("not found skeleton");
                        return;
                    };

                    // Skinned sets the skeleton root as the parent
                    commands
                        .entity(nifscene_root)
                        .insert(ChildOf(*skeleton_entity));
                    commands.entity(nifscene_root).remove::<AttachmentType>();
                    for trishape_entity in pending_skins.pending.iter() {
                        let Ok(mut pending_skin) = pending_nif_skin_q.get_mut(*trishape_entity)
                        else {
                            continue;
                        };
                        let mut joints_vec = Vec::with_capacity(pending_skin.bone_names.len());
                        let mut missing_bone = false;

                        for bone_name in &pending_skin.bone_names {
                            if let Some(bone_data) = skeleton.get_bone_by_name(bone_name) {
                                joints_vec.push(bone_data.entity);
                            } else {
                                warn!(
                                    "Skinning error: Bone '{}' not found in skeleton.",
                                    bone_name
                                );
                                missing_bone = true;
                                break;
                            }
                        }

                        if let Ok((mesh3d, _material)) = materials_query.get(*trishape_entity) {
                            if let Some(mesh) = meshes.get(&mesh3d.0) {
                                if !missing_bone
                                    && joints_vec.len() == pending_skin.bone_names.len()
                                {
                                    commands.entity(*trishape_entity).insert(SkinnedMesh {
                                        inverse_bindposes: pending_skin.inverse_bindposes.clone(),
                                        joints: joints_vec,
                                    });
                                    if mesh.contains_attribute(Mesh::ATTRIBUTE_JOINT_INDEX) {
                                        continue;
                                    }
                                    let mut new_mesh = mesh.clone();

                                    new_mesh.insert_attribute(
                                        Mesh::ATTRIBUTE_JOINT_INDEX,
                                        VertexAttributeValues::Uint16x4(std::mem::take(
                                            &mut pending_skin.joint_indices,
                                        )),
                                    );
                                    new_mesh.insert_attribute(
                                        Mesh::ATTRIBUTE_JOINT_WEIGHT,
                                        VertexAttributeValues::Float32x4(std::mem::take(
                                            &mut pending_skin.joint_weights,
                                        )),
                                    );
                                    let new_mesh_handle = meshes.add(new_mesh);
                                    commands
                                        .entity(*trishape_entity)
                                        .insert(Mesh3d(new_mesh_handle));
                                    commands.entity(*trishape_entity).remove::<PendingNifSkin>();
                                }
                            }
                        }
                    }
                }
            }
            _ => { // DoubleSidedRigid is only used as a marker for the user
            }
        }
    }
}
