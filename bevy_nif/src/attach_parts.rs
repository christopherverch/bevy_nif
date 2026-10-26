use crate::{
    loader::{NeedsSkeletonMapping, NifNodeIndex, PendingNifSkins},
    loader_spawners::{NeedsNifSkinMapping, PendingNifSkin},
    skeleton::{Skeleton, SkeletonMap},
};

use bevy_asset::Assets;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    observer::On,
    query::{With, Without},
    system::{Commands, Query, ResMut},
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
pub fn handle_needs_nifskin_mapping(
    event: On<WorldInstanceReady>,
    pending_nif_skins: Query<(Entity, &ChildOf, &mut PendingNifSkins), With<NeedsNifSkinMapping>>,
    attach_query: Query<(Entity, &AttachmentType, &Children)>,
    mut commands: Commands,
) {
    for (pending_nif_entity, child_of, mut pending_nif_skins) in pending_nif_skins {
        if child_of.0 == event.entity {
            if let Ok((_, attachment_type, _)) = attach_query.get(event.entity) {
                match attachment_type {
                    AttachmentType::Skinned { skeleton_entity } => {
                        pending_nif_skins.skeleton_entity = Some(*skeleton_entity);
                        commands
                            .entity(pending_nif_entity)
                            .remove::<NeedsNifSkinMapping>();
                    }
                    _ => {}
                }
            }
        }
    }
}
pub fn insert_into_skeleton_map(
    event: On<WorldInstanceReady>,
    skeletons_need_setup: Query<(Entity, &mut Skeleton, &ChildOf), With<NeedsSkeletonMapping>>,
    mut skeleton_map: ResMut<SkeletonMap>,
    attach_query: Query<(Entity, &AttachmentType, &Children)>,
    transforms: Query<&mut Transform>,
    materials_query: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>,
    materials: ResMut<Assets<StandardMaterial>>,
    nif_node_index_q: Query<&NifNodeIndex>,
    meshes: ResMut<Assets<Mesh>>,
    pending_nif_skins_q: Query<&mut PendingNifSkins, Without<NeedsNifSkinMapping>>,
    pending_nif_skin_q: Query<&mut PendingNifSkin>,
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
            attach_parts(
                attach_query,
                &skeleton_map,
                commands,
                transforms,
                materials_query,
                materials,
                nif_node_index_q,
                meshes,
                pending_nif_skins_q,
                pending_nif_skin_q,
            );
            break;
        }
    }
}

pub fn attach_parts(
    attach_query: Query<(Entity, &AttachmentType, &Children)>,
    skeleton_map: &SkeletonMap,
    mut commands: Commands,
    mut transforms: Query<&mut Transform>,
    materials_query: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    nif_node_index_q: Query<&NifNodeIndex>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut pending_nif_skins_q: Query<&mut PendingNifSkins, Without<NeedsNifSkinMapping>>,
    mut pending_nif_skin_q: Query<&mut PendingNifSkin>,
) {
    for (nifscene_root, attach_type, children) in attach_query {
        if let AttachmentType::Rigid {
            skeleton_entity,
            target_bone,
        } = attach_type
        {
            let Some(skeleton) = skeleton_map.entity_to_skeleton.get(skeleton_entity) else {
                return;
            };
            let Some(skeleton_bone) = skeleton.get_bone_by_name(target_bone) else {
                error!("nif tried to attach to nonexistant bone!");
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

            commands
                .entity(nifscene_root)
                .insert(ChildOf(skeleton_bone.entity));

            commands.entity(nifscene_root).remove::<AttachmentType>();
        } else {
            match attach_type {
                AttachmentType::Skinned { skeleton_entity } => {
                    for pending_skins in pending_nif_skins_q.iter_mut() {
                        if pending_skins.skeleton_entity != Some(*skeleton_entity) {
                            continue;
                        }

                        let Some(skeleton) = skeleton_map.entity_to_skeleton.get(skeleton_entity)
                        else {
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
                                if let Some(mut mesh) = meshes.get_mut(&mesh3d.0) {
                                    if !missing_bone
                                        && joints_vec.len() == pending_skin.bone_names.len()
                                    {
                                        dbg!("inserting");
                                        commands.entity(*trishape_entity).insert(SkinnedMesh {
                                            inverse_bindposes: pending_skin
                                                .inverse_bindposes
                                                .clone(),
                                            joints: joints_vec,
                                        });

                                        mesh.insert_attribute(
                                            Mesh::ATTRIBUTE_JOINT_INDEX,
                                            VertexAttributeValues::Uint16x4(std::mem::take(
                                                &mut pending_skin.joint_indices,
                                            )),
                                        );
                                        mesh.insert_attribute(
                                            Mesh::ATTRIBUTE_JOINT_WEIGHT,
                                            VertexAttributeValues::Float32x4(std::mem::take(
                                                &mut pending_skin.joint_weights,
                                            )),
                                        );
                                        commands
                                            .entity(*trishape_entity)
                                            .remove::<PendingNifSkin>();
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
}
