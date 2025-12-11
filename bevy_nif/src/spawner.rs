use crate::attach_parts::AttachmentType;
use crate::nif_animation::SkeletonMap;
use crate::spawning_ni_helpers::{process_nimaterialproperty, process_nitexturingproperty};
use crate::{NeedsNifPhysics, skeleton::*};
use bevy::asset::{Assets, Handle};
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::mesh::VertexAttributeValues;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy_rapier3d::prelude::{Collider, RigidBody};
use nif::loader::ConsumedNiType;
use nif::loader::NiKey;
use nif::loader::Nif;
use nif::{BoundData, NiSkinInstance, NiType};
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::f32::consts::PI;
#[derive(Component)]
pub struct MainNifSkeleton;
#[derive(Component)]
pub struct NeedsNifAnimator {
    pub handle: Handle<Nif>,
    pub skeleton_id: u64,
}
#[allow(dead_code)]
#[derive(Event, Clone, Debug)]
pub struct NifInstantiated {
    pub handle: Handle<Nif>,
    pub root_entity: Entity,
    pub skeleton_id_opt: Option<u64>,
}
#[allow(dead_code)]
#[derive(Component)]
pub struct LoadedNifScene(pub Handle<Nif>);
#[derive(Resource, Default, Debug, Component)]
pub struct NifScene(pub Handle<Nif>);
//for nif->bevy coordinates
pub const MESH_ROTATION: Quat = Quat::from_xyzw(0.0, 0.70710677, 0.70710677, 0.0);

pub fn spawn_nif_scenes(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    nif_assets: Res<Assets<Nif>>,
    asset_server: Res<AssetServer>,
    mut new_scenes: Query<
        (
            Entity,
            &mut Transform,
            &NifScene,
            Option<&mut AttachmentType>,
        ),
        Without<LoadedNifScene>,
    >,
    mut inverse_bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut skeleton_map_res: ResMut<SkeletonMap>,
) {
    if new_scenes.is_empty() {
        return;
    }
    // is_main_skeleton is just based on if the asset_path contains base_anim.nif
    // default to false
    let mut is_main_skeleton = false;
    let Some((entity, asset_handle, nif_scene_component, target_skeleton_id_opt)) =
        new_scenes.iter_mut().find_map(
            |(entity, mut transform, nif_scene_component, attachment_type_opt)| {
                let asset_handle = &nif_scene_component.0;
                let asset_path = asset_handle.path()?.to_string();

                is_main_skeleton = asset_path.contains("base_anim.nif");
                let target_skeleton_id_opt =
                    attachment_type_opt.map(|a| a.get_target_skeleton_id());

                if !is_main_skeleton {
                    if let Some(id) = target_skeleton_id_opt {
                        let exists = skeleton_map_res.root_skeleton_entity_map.contains_key(&id);
                        if !exists {
                            // If it's not a skeleton asset, and this asset relies on a skeleton
                            // that doesn't exist (yet?), skip for now and try the next asset
                            return None;
                        }
                    }
                }

                if is_main_skeleton {
                    // TODO:: maybe this should happen in the loader, modifying the root node?
                    transform.rotation =
                        Quat::from_rotation_x(-FRAC_PI_2) * Quat::from_rotation_z(PI);
                }

                Some((
                    entity,
                    asset_handle,
                    nif_scene_component,
                    target_skeleton_id_opt,
                ))
            },
        )
    else {
        println!("No suitable NIF scene found.");
        return;
    };
    let Some(nif) = nif_assets.get(&nif_scene_component.0) else {
        println!("Nif hasn't finished loading");
        return;
    };

    let mut skeleton = Skeleton::new();
    let already_spawned_nodes = HashMap::new();
    let ninodes_with_bvs = Vec::new();
    let mut spawn_context = SpawnContext {
        target_skeleton_id_opt,
        is_main_skeleton,
        asset_server: &asset_server,
        already_spawned_nodes,
        ninodes_with_bvs,
    };
    // spawn all the root nodes, parenting to the root entity
    for (index, current_node) in nif.roots.iter().enumerate() {
        let root_node_entity = commands
            .spawn((
                Transform::default(),
                Visibility::Inherited,
                Name::new(format!("NifScene {:?}_{}", asset_handle.id(), index)),
            ))
            .id();
        commands.entity(entity).add_child(root_node_entity);
        let current_parent_entity = root_node_entity;

        new_spawn_nif_node_recursive(
            nif,
            &mut spawn_context,
            current_node.key,
            current_parent_entity,
            None,
            &mut skeleton,
            &mut skeleton_map_res,
            &mut materials,
            &mut meshes,
            &mut inverse_bindposes,
            &mut commands,
        );
    }

    commands.trigger(NifInstantiated {
        handle: asset_handle.clone(),
        root_entity: entity,
        skeleton_id_opt: target_skeleton_id_opt,
    });
    commands
        .entity(entity)
        .insert(NeedsNifPhysics(spawn_context.ninodes_with_bvs));
    commands
        .entity(entity)
        .insert(LoadedNifScene(asset_handle.clone()));
    if is_main_skeleton {
        if let Some(target_skeleton_id) = target_skeleton_id_opt {
            skeleton_map_res
                .root_skeleton_entity_map
                .insert(target_skeleton_id, entity);
            skeleton_map_res
                .skeletons
                .insert(target_skeleton_id, skeleton);

            commands.entity(entity).insert(NeedsNifAnimator {
                handle: asset_handle.clone(),
                skeleton_id: target_skeleton_id,
            });
            commands.entity(entity).insert(MainNifSkeleton);
        }
    }
}

struct SpawnContext<'a> {
    target_skeleton_id_opt: Option<u64>,
    is_main_skeleton: bool,
    asset_server: &'a AssetServer,
    already_spawned_nodes: HashMap<NiKey, Entity>,
    ninodes_with_bvs: Vec<(Entity, NiKey)>,
}
fn new_spawn_nif_node_recursive<'a>(
    nif: &Nif,
    spawn_context: &mut SpawnContext<'a>,
    current_key: NiKey,
    parent_entity: Entity,
    parent_bone_name_opt: Option<&str>,
    skeleton: &mut Skeleton,
    skeleton_map: &mut ResMut<SkeletonMap>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    inverse_bindposes: &mut ResMut<Assets<SkinnedMeshInverseBindposes>>,
    commands: &mut Commands,
) {
    if spawn_context
        .already_spawned_nodes
        .contains_key(&current_key)
        == true
    {
        // We already spawned this node somewhere else (such as NiSkinInstance), skip it
        return;
    };
    let Some(ni_type) = nif.objects.get(current_key) else {
        return;
    };
    match ni_type {
        NiType::NiNode(ni_node) => {
            let nif_transform = ni_node;
            let rot_mat = Mat3::from_cols(
                Vec3::new(
                    nif_transform.rotation.x_axis[0],
                    nif_transform.rotation.y_axis[0],
                    nif_transform.rotation.z_axis[0],
                ), // First Column
                Vec3::new(
                    nif_transform.rotation.x_axis[1],
                    nif_transform.rotation.y_axis[1],
                    nif_transform.rotation.z_axis[1],
                ), // Second Column
                Vec3::new(
                    nif_transform.rotation.x_axis[2],
                    nif_transform.rotation.y_axis[2],
                    nif_transform.rotation.z_axis[2],
                ), // Third Column
            );
            let bevy_transform = Transform {
                translation: ni_node.translation,
                rotation: Quat::from_mat3(&rot_mat),
                scale: Vec3::splat(ni_node.scale),
            };
            let new_ninode_entity = commands
                .spawn((bevy_transform, Name::new(ni_node.name.clone())))
                .id();
            if let Some(bounding_volume) = &ni_node.bounding_volume {
                spawn_context
                    .ninodes_with_bvs
                    .push((new_ninode_entity, current_key));
                match &bounding_volume.bound_data {
                    BoundData::NiBoxBV(box_bv) => {
                        let rigid_body_type = RigidBody::KinematicPositionBased;
                        commands.entity(new_ninode_entity).insert((
                            rigid_body_type,
                            Collider::cuboid(box_bv.extents.x, box_bv.extents.y, box_bv.extents.z),
                            Transform::from_translation(box_bv.center), // The local offset
                        ));
                    }

                    BoundData::NiSphereBV(sphere_bv) => {
                        todo!()
                    }
                    BoundData::NiUnionBV(sphere_bv) => {
                        todo!()
                    }
                }
            }

            spawn_context
                .already_spawned_nodes
                .insert(current_key, new_ninode_entity);
            let mut current_bone_name_opt = None;
            if spawn_context.is_main_skeleton {
                let formatted_name = format!("skeleton {}", ni_node.name);
                commands
                    .entity(new_ninode_entity)
                    .insert(Name::new(formatted_name));

                skeleton.add_bone(
                    new_ninode_entity,
                    ni_node.name.to_string(), // Use the raw NIF name
                    parent_bone_name_opt,
                );
                current_bone_name_opt = Some(ni_node.name.as_str());
            }
            commands.entity(parent_entity).add_child(new_ninode_entity);
            for child in &ni_node.children {
                new_spawn_nif_node_recursive(
                    nif,
                    spawn_context,
                    child.key,
                    new_ninode_entity,
                    current_bone_name_opt,
                    skeleton,
                    skeleton_map,
                    materials,
                    meshes,
                    inverse_bindposes,
                    commands,
                );
            }
        }
        NiType::NiTriShape(ni_trishape) => {
            // Create the entity and set up the name
            let new_nitrishape_entity = commands.spawn(ni_trishape.transform()).id();
            let name_ref: &str = &ni_trishape.name;
            let formatted_name = format!("NiTriShape: {:?}", name_ref);
            // Make shadow invisible (or if it's the main skeleton bones)
            if name_ref == "Tri Shadow"
                || name_ref == "Tri QuadPatch01"
                || spawn_context.is_main_skeleton
            {
                commands
                    .entity(new_nitrishape_entity)
                    .insert(Visibility::Hidden);
            }
            commands
                .entity(new_nitrishape_entity)
                .insert(Name::new(formatted_name));

            let Some(consumed_ni_type) = nif.block_assets.get(&ni_trishape.geometry_data.key)
            else {
                warn!("NiTriShape missing mesh!");
                return;
            };

            let mesh_handle = match consumed_ni_type {
                ConsumedNiType::NiTriShapeData(mesh_handle) => {
                    commands
                        .entity(new_nitrishape_entity)
                        .insert(Mesh3d(mesh_handle.clone()));
                    mesh_handle
                }
            };
            // Loop through properties such as material and textures
            let ni_properties = &ni_trishape.properties;
            let mut material_opt: Option<StandardMaterial> = None;
            let mut texture_handle_opt = None;
            for property in ni_properties {
                if let Some(ni_property) = nif.objects.get(property.key) {
                    match ni_property {
                        NiType::NiTexturingProperty(tex_prop) => {
                            texture_handle_opt = process_nitexturingproperty(
                                tex_prop,
                                nif,
                                spawn_context.asset_server,
                            );
                        }
                        NiType::NiMaterialProperty(mat_prop) => {
                            material_opt = Some(process_nimaterialproperty(mat_prop));
                        }
                        _ => {}
                    }
                }
            }
            // Assemble the final material, if there was one
            if let Some(mut material) = material_opt {
                material.base_color_texture = texture_handle_opt.take();
                material.alpha_mode = AlphaMode::Mask(1.0);
                material.cull_mode = None;
                let material_h = materials.add(material);
                commands
                    .entity(new_nitrishape_entity)
                    .insert(MeshMaterial3d(material_h));
            }
            commands
                .entity(parent_entity)
                .add_child(new_nitrishape_entity);
            let skin_key = ni_trishape.skin_instance.key;
            if let Some(ni_skin_instance) = nif.objects.get(skin_key) {
                match ni_skin_instance {
                    NiType::NiSkinInstance(skin_instance) => {
                        apply_skin_instance(
                            nif,
                            spawn_context,
                            skin_instance,
                            new_nitrishape_entity,
                            parent_entity,
                            parent_bone_name_opt,
                            skeleton,
                            skeleton_map,
                            mesh_handle,
                            meshes,
                            materials,
                            inverse_bindposes,
                            commands,
                        );
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

/// Apply Skinning Attributes & Spawn Skeleton (if needed)
fn apply_skin_instance(
    nif: &Nif,
    spawn_context: &mut SpawnContext,
    skin_instance: &NiSkinInstance,
    current_entity: Entity,
    parent_entity: Entity,
    parent_bone_name_opt: Option<&str>,
    skeleton: &mut Skeleton,
    skeleton_map: &mut ResMut<SkeletonMap>,
    mesh_handle: &Handle<Mesh>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    inverse_bindposes: &mut ResMut<Assets<SkinnedMeshInverseBindposes>>,
    commands: &mut Commands,
) {
    let skin_data_key = skin_instance.data.key;
    let Some(skin_data) = nif.objects.get(skin_data_key) else {
        return;
    };
    let skin_data = match skin_data {
        NiType::NiSkinData(sd) => sd,
        _ => {
            return;
        }
    };
    // 1. Add vertex attributes
    if let Some(mesh) = meshes.get_mut(mesh_handle) {
        if let Some(vertex_count) = mesh.attribute(Mesh::ATTRIBUTE_POSITION).map(|a| a.len()) {
            // ... (Initialize joint_indices, joint_weights, vertex_bone_counts) ...
            let mut joint_indices: Vec<[u16; 4]> = vec![[0, 0, 0, 0]; vertex_count];
            let mut joint_weights: Vec<[f32; 4]> = vec![[0.0, 0.0, 0.0, 0.0]; vertex_count];
            let mut vertex_bone_counts: Vec<u8> = vec![0; vertex_count];
            // ... (Loop sd.bone_list, loop weight_data, populate indices/weights) ...

            for (bone_list_idx, bone_data) in skin_data.bone_data.iter().enumerate() {
                if bone_list_idx >= 256 {
                    continue;
                }
                for (index, weight) in &bone_data.vertex_weights {
                    let vertex_index = *index as usize;
                    if let Some(slot) = vertex_bone_counts.get_mut(vertex_index) {
                        if *slot < 4 {
                            joint_indices[vertex_index][*slot as usize] = bone_list_idx as u16;
                            joint_weights[vertex_index][*slot as usize] = *weight;
                            *slot += 1;
                        }
                    } else {
                        warn!("Invalid vertex index {} in skin data", vertex_index);
                    }
                }
            }
            // ... (Normalize weights) ...
            for i in 0..vertex_count {
                let sum: f32 = joint_weights[i].iter().sum();
                if sum > 1e-6 {
                    for j in 0..4 {
                        joint_weights[i][j] /= sum;
                    }
                }
            }
            // Insert Bevy vertex attributes
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_JOINT_INDEX,
                VertexAttributeValues::Uint16x4(joint_indices),
            );
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_JOINT_WEIGHT,
                VertexAttributeValues::Float32x4(joint_weights),
            );
        } else {
            warn!(
                "   Could not apply skinning attributes: Mesh for {:?} missing positions?",
                skin_instance.root.key,
            );
        }
    } else {
        warn!(
            "   Could not get mutable mesh asset for handle {:?}",
            mesh_handle.id()
        );
    }

    // 2. Spawn Skeleton Hierarchy
    if !spawn_context.is_main_skeleton {
        // if not the main skeleton, but we have skinning data, we wouldn't set up this asset unless
        // the skeleton was already set up correctly, so we can just assume it's set up
    } else {
        let root_key = skin_instance.root.key;
        // If the root wasn't spawned yet, spawn it
        if !spawn_context.already_spawned_nodes.contains_key(&root_key) {
            new_spawn_nif_node_recursive(
                nif,
                spawn_context,
                root_key,
                //TODO:: Is this right? I think we might have to spawn the whole tree first
                //since this just spawns it at the current parent level arbitrarily
                parent_entity,
                parent_bone_name_opt,
                skeleton,
                skeleton_map,
                materials,
                meshes,
                inverse_bindposes,
                commands,
            );
        }
    }
    // 3a. Create Inverse Bind Pose Asset
    let mut ibp_matrices = Vec::with_capacity(skin_data.bone_data.len());
    for bone_data in &skin_data.bone_data {
        // Convert NIF transform -> Bevy Transform -> Bevy Mat4
        // Assumes bone_data.bone_transform IS the inverse bind pose
        let bevy_transform = Transform {
            translation: bone_data.translation,
            rotation: Quat::from_mat3(&bone_data.rotation),
            scale: Vec3::splat(bone_data.scale),
        };
        ibp_matrices.push(bevy_transform.to_matrix());
    }
    // Create and add the asset
    let ibp_handle = inverse_bindposes.add(SkinnedMeshInverseBindposes::from(ibp_matrices));

    // 3b. Build Joints Vec<Entity>
    let mut joints_vec: Vec<Entity> = Vec::with_capacity(skin_instance.bones.len());
    let mut missing_bone = false;
    if !spawn_context.is_main_skeleton {
        // --- ATTACHABLE NIF LOGIC ---
        // `base_skeleton_map_holder` is `&ActiveSkeletonBones`
        if let Some(target_skeleton_id) = spawn_context.target_skeleton_id_opt {
            if let Some(skeleton) = &skeleton_map.skeletons.get(&target_skeleton_id) {
                for (bone_order_idx, bone_link_in_current_nif) in
                    skin_instance.bones.iter().enumerate()
                {
                    if let Some(bone_object_nitype) = nif.objects.get(bone_link_in_current_nif.key)
                    {
                        let bone_object = match bone_object_nitype {
                            NiType::NiNode(ni_node) => ni_node,
                            _ => {
                                // Should be unreachable
                                warn!("NiSkinInstance bone linked to non-NiAVObject!");
                                warn!("{:?}", bone_object_nitype);
                                return;
                            }
                        };
                        let bone_name = &bone_object.name;
                        if let Some(bone_data) = skeleton.get_bone_by_name(bone_name) {
                            joints_vec.push(bone_data.entity);
                        } else {
                            warn!(
                                "Attachable TriShape root {:?}: Bone '{}' not found in active base skeleton map.",
                                skin_instance.root,
                                &format!("{}", bone_name),
                            );
                            missing_bone = true;
                            break;
                        }
                    } else {
                        warn!(
                            "Attachable TriShape root {:?}: Missing bone link at order index {} in SkinInstance.",
                            skin_instance.root, bone_order_idx
                        );
                        missing_bone = true;
                        break;
                    }
                }
            }
        }
    } else {
        for (i, bone_link) in skin_instance.bones.iter().enumerate() {
            // Look up the Entity spawned for this bone's NiNode index
            if let Some(bone_entity) = spawn_context.already_spawned_nodes.get(&bone_link.key) {
                joints_vec.push(*bone_entity);
            } else {
                warn!(
                    "   Bone node link #{} (key {:?}) from SkinInstance root {:?} not found in spawned entity map! Cannot add SkinnedMesh.",
                    i, bone_link.key, skin_instance.root
                );
                missing_bone = true;
                break;
            }
        }
    }
    // 3c. Add SkinnedMesh Component (if all bones found)
    if !missing_bone && joints_vec.len() == skin_data.bone_data.len() {
        commands.entity(current_entity).insert(SkinnedMesh {
            inverse_bindposes: ibp_handle,
            joints: joints_vec,
        });
    } else {
        warn!(
            "   Failed to add SkinnedMesh component due to missing bone entity or count mismatch."
        );
    }
}
