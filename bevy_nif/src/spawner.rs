use crate::attach_parts::AttachmentType;
use crate::nif_animation::SkeletonMap;
use crate::skeleton::*;
use crate::spawning_ni_helpers::{process_nimaterialproperty, process_nitexturingproperty};
use bevy::asset::{Assets, Handle};
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy::render::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::render::mesh::{Mesh, VertexAttributeValues};
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
        let already_spawned_nodes = HashMap::new();
        let mut spawn_context = SpawnContext {
            target_skeleton_id_opt,
            is_main_skeleton,
            asset_server: &asset_server,
            already_spawned_nodes,
        };
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
        NiType::NiKeyframeController(kfc) => {}
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
        ibp_matrices.push(bevy_transform.compute_matrix());
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
/*
       fn spawn_nif_node_recursive(
       commands: &mut Commands,
       skeleton_map: &SkeletonMap,
       nif_index: usize,
       parent_entity: Entity,
       block_map: &HashMap<usize, &ParsedBlock>,
       entity_map: &mut HashMap<usize, Entity>,
       mesh_handles: &HashMap<usize, Handle<Mesh>>,
       material_handles: &HashMap<usize, Handle<StandardMaterial>>,
       texture_info_map: &HashMap<usize, NifTextureInfo>,
       materials: &mut ResMut<Assets<StandardMaterial>>,
       asset_server: &Res<AssetServer>,
       meshes: &mut ResMut<Assets<Mesh>>,
       inverse_bindposes: &mut ResMut<Assets<SkinnedMeshInverseBindposes>>,
       block: &ParsedBlock,
       is_main_skeleton: bool,
       target_skeleton_id_opt: Option<u64>,
       skeleton: &mut Skeleton,
       parent_bone_name_opt: Option<&str>,
       ) {
       if entity_map.contains_key(&nif_index) {
       return;
       } // Avoid cycles/duplicates

       let bevy_transform = match block {
       ParsedBlock::Node(data) => convert_nif_transform(&data.av_base.transform),
       ParsedBlock::TriShape(data) => convert_nif_transform(&data.av_base.transform),
       _ => Transform::IDENTITY,
       };
       let current_entity_id = commands
       .spawn((
       bevy_transform,
       Visibility::Inherited, // Keep basic visibility
       Name::new(format!("NifBlock_{}", nif_index)), // Initial name
       ))
       .id();
           entity_map.insert(nif_index, current_entity_id); // Track entity immediately

           let mut should_keep_entity = true;
           let mut current_bone_name_opt: Option<&str> = None;
           match block {
           ParsedBlock::Node(node_data) => {
    // Insert Name using commands.entity()
    let name_with_ninode = format!("NiNode: {}", node_data.name());
    commands
    .entity(current_entity_id)
    .insert(Name::new(name_with_ninode.clone()));
               // set root of entity (Bip01) to origin since it can be weirdly offset
               if is_main_skeleton {
               skeleton.add_bone(
               current_entity_id,
               node_data.name().to_string(), // Use the raw NIF name
               parent_bone_name_opt,
               );
               current_bone_name_opt = Some(&node_data.name());
               }

    // Recurse for children
    for child_link in &node_data.children {
    if let Some(child_index) = child_link {
    // Get child block data BEFORE recursing
    if let Some(child_block_data) = block_map.get(child_index) {
    spawn_nif_node_recursive(
    commands,
    skeleton_map,
    *child_index,
    current_entity_id, // Current node is parent
    block_map,
    entity_map,
    mesh_handles,
    material_handles,
    texture_info_map,
    materials,
    asset_server,
    meshes,
    inverse_bindposes,
    child_block_data,
    is_main_skeleton,
    target_skeleton_id_opt,
    skeleton,
    current_bone_name_opt,
    );
} else {
    warn!("Node {}: Child link {} invalid", nif_index, child_index);
    }
}
}
}
ParsedBlock::TriShape(trishape_data) => {
    let name_ref: &str = trishape_data.av_base.net_base.name();
    let formatted_name = format!("NiTriShape: {:?}", name_ref);
    if name_ref == "Tri Shadow" || name_ref == "Tri QuadPatch01" || is_main_skeleton {
        commands
            .entity(current_entity_id)
            .insert(Visibility::Hidden);
    }
    commands
        .entity(current_entity_id)
        .insert(Name::new(formatted_name));
    // --- Determine Material and Apply Textures ---
    let mut base_material_handle: Option<Handle<StandardMaterial>> = None;
    let mut final_texture_info: Option<NifTextureInfo> = None;
    let mut final_alpha_mode: Option<AlphaMode> = None; // Store potential alpha override

    // Find first MaterialProperty and first TexturingProperty linked
    for prop_link in &trishape_data.av_base.properties {
        if let Some(prop_idx) = prop_link {
            match block_map.get(prop_idx) {
                Some(ParsedBlock::MaterialProperty(_))
                    if base_material_handle.is_none() =>
                    {
                        base_material_handle = material_handles.get(prop_idx).cloned();
                    }
                Some(ParsedBlock::TexturingProperty(_)) if final_texture_info.is_none() => {
                    final_texture_info = texture_info_map.get(prop_idx).cloned();
                }
                Some(ParsedBlock::AlphaProperty(alpha_prop)) => {
                    let flags = alpha_prop.flags;
                    let threshold = alpha_prop.threshold;
                    // Check flags according to common NIF specs:
                    let enable_testing = (flags & 0x0200) != 0; // Bit 9 usually enables testing
                    let enable_blending = (flags & 0x0001) != 0; // Bit 0 usually enables blending

                    if enable_testing {
                        // Use Mask mode if testing is enabled
                        let mask_cutoff = threshold as f32 / 255.0;
                        final_alpha_mode = Some(AlphaMode::Mask(mask_cutoff));
                        // Alpha-masked materials often benefit from being double-sided
                        // might want to handle this later when applying the final material
                        // e.g., set `final_material.double_sided = true;`
                    } else if enable_blending {
                        // Use Blend mode if blending is enabled (and testing is not)
                        final_alpha_mode = Some(AlphaMode::Blend);
                        // Blended materials often benefit from being double-sided
                        // e.g., set `final_material.double_sided = true;`
                    } else {
                        // Otherwise, it's opaque
                        final_alpha_mode = Some(AlphaMode::Opaque);
                    }
                }
                _ => {}
            }
        }
    }

    // --- Get Mesh ---
    let Some(data_link) = trishape_data.data_link else {
        warn!("TriShape {} missing data link.", nif_index);
        commands.entity(current_entity_id).despawn();
        entity_map.remove(&nif_index);
        return;
    };
    let Some(mesh_handle) = mesh_handles.get(&data_link).cloned() else {
        warn!(
            "Mesh handle not found for data link {} from TriShape {}.",
            data_link, nif_index
        );
        commands.entity(current_entity_id).despawn();
        entity_map.remove(&nif_index);
        return;
    };

    // --- Find Associated Skinning Data ---
    let mut skin_instance_data: Option<&NiSkinInstance> = None;
    let mut skin_data: Option<&NiSkinData> = None;
    let mut skin_instance_block_index: Option<usize> = None;

    // Find linked NiSkinInstance (adjust this logic if link isn't controller_link)
    let controller_link = trishape_data.av_base.net_base.controller_link;
    if let Some(si_link) = trishape_data.skin_link {
        // Assuming this field exists now
        // Convert link to usize for HashMap lookup if needed (depends on your types)
        let si_link_idx = si_link as usize;
        if let Some(ParsedBlock::SkinInstance(si)) = block_map.get(&si_link_idx) {
            skin_instance_data = Some(si);
            skin_instance_block_index = Some(si_link_idx);
            // Now find its associated SkinData
            if let Some(sd_link) = si.data {
                // Convert sd_link to usize if needed
                let sd_link_idx = sd_link as usize;
                if let Some(ParsedBlock::SkinData(sd)) = block_map.get(&sd_link_idx) {
                    skin_data = Some(sd);
                } else {
                    warn!(
                        " -> SkinInstance {} links to SkinData {} which is not found or wrong type.",
                        si_link_idx, sd_link
                    );
                }
            } else {
                warn!(" -> SkinInstance {} has no link to SkinData.", si_link_idx);
            }
        } else {
            warn!(
                "TriShape {}: Direct skin_instance_link {} is invalid or points to wrong block type.",
                nif_index, si_link
            );
        }
    }
    if skin_instance_data.is_none() {
        for prop_link_opt in &trishape_data.av_base.properties {
            if let Some(prop_idx) = prop_link_opt {
                // Check if the block pointed to by this property link is a SkinInstance
                if let Some(ParsedBlock::SkinInstance(si)) = block_map.get(prop_idx) {
                    skin_instance_data = Some(si);
                    skin_instance_block_index = Some(*prop_idx);
                    // Now that we have the SkinInstance, try to find its associated SkinData
                    if let Some(sd_link) = si.data {
                        if let Some(ParsedBlock::SkinData(sd)) = block_map.get(&sd_link) {
                            skin_data = Some(sd);
                        } else {
                            warn!(
                                "      -> SkinInstance {} links to SkinData {} which is not found or wrong type",
                                prop_idx, sd_link
                            );
                        }
                    } else {
                        warn!("      -> SkinInstance {} has no link to SkinData", prop_idx);
                    }
                    // Found it via properties, no need to check further properties for SkinInstance
                    break;
                }
                // else: This property wasn't a SkinInstance, continue checking others
            }
        }
    }
    if let Some(link_idx) = controller_link {
        if let Some(ParsedBlock::SkinInstance(si)) = block_map.get(&link_idx) {
            skin_instance_data = Some(si);
            skin_instance_block_index = Some(link_idx); // Store for logging
            if let Some(sd_link) = si.data {
                if let Some(ParsedBlock::SkinData(sd)) = block_map.get(&sd_link) {
                    skin_data = Some(sd);
                } else {
                    warn!(
                        "SkinInstance {} links to SkinData {} which is not found or wrong type",
                        link_idx, sd_link
                    );
                }
            } else {
                warn!("SkinInstance {} has no link to SkinData", link_idx);
            }
        } // Add else if needed to check properties list?
    } else {
    }

    // --- Apply Skinning Attributes & Spawn Skeleton (if needed) ---
    if let (Some(sd), Some(si), Some(si_index)) =
        (skin_data, skin_instance_data, skin_instance_block_index)
    {
        // 1. Add vertex attributes
        if let Some(mesh) = meshes.get_mut(&mesh_handle) {
            if let Some(vertex_count) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION).map(|a| a.len())
            {
                // ... (Initialize joint_indices, joint_weights, vertex_bone_counts) ...
                let mut joint_indices: Vec<[u16; 4]> = vec![[0, 0, 0, 0]; vertex_count];
                let mut joint_weights: Vec<[f32; 4]> =
                    vec![[0.0, 0.0, 0.0, 0.0]; vertex_count];
                let mut vertex_bone_counts: Vec<u8> = vec![0; vertex_count];
                // ... (Loop sd.bone_list, loop weight_data, populate indices/weights) ...
                for (bone_list_idx, bone_data) in sd.bone_list.iter().enumerate() {
                    if bone_list_idx >= 256 {
                        continue;
                    }
                    for weight_data in &bone_data.vertex_weights {
                        let vertex_index = weight_data.index as usize;
                        if let Some(slot) = vertex_bone_counts.get_mut(vertex_index) {
                            if *slot < 4 {
                                joint_indices[vertex_index][*slot as usize] =
                                    bone_list_idx as u16;
                                joint_weights[vertex_index][*slot as usize] =
                                    weight_data.weight;
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
                    "   Could not apply skinning attributes: Mesh for {} missing positions?",
                    nif_index
                );
            }
        } else {
            warn!(
                "   Could not get mutable mesh asset for handle {:?}",
                mesh_handle.id()
            );
        }

        // 2. Spawn Skeleton Hierarchy
        let mut skeleton_ready = false;
        if !is_main_skeleton {
            // if not the main skeleton, we can only get here if the skeleton is already
            // set up correctly
            skeleton_ready = true;
        } else {
            if let Some(skeleton_root_index) = si.skeleton_root {
                if !entity_map.contains_key(&skeleton_root_index) {
                    if let Some(root_block_data) = block_map.get(&skeleton_root_index) {
                        spawn_nif_node_recursive(
                            commands,
                            skeleton_map,
                            skeleton_root_index,
                            parent_entity, // Parent skeleton to same parent as mesh
                            block_map,
                            entity_map,
                            mesh_handles,
                            material_handles,
                            texture_info_map,
                            materials,
                            asset_server,
                            meshes,
                            inverse_bindposes,
                            root_block_data, // *** Pass skeleton root's block data ***
                            is_main_skeleton,
                            target_skeleton_id_opt,
                            skeleton,
                            current_bone_name_opt,
                        );
                        skeleton_ready = true;
                    } else {
                        warn!("Skeleton root link invalid");
                    }
                } else {
                    skeleton_ready = true;
                }
            } else {
                warn!("SkinInstance has no skeleton root link");
            }
        }
        if skeleton_ready {
            // 3a. Create Inverse Bind Pose Asset
            let mut ibp_matrices = Vec::with_capacity(sd.bone_list.len());
            for bone_data in &sd.bone_list {
                // Convert NIF transform -> Bevy Transform -> Bevy Mat4
                // Assumes bone_data.bone_transform IS the inverse bind pose
                let bevy_transform = convert_nif_transform(&bone_data.bone_transform);
                ibp_matrices.push(bevy_transform.compute_matrix());
            }
            // Create and add the asset
            let ibp_handle =
                inverse_bindposes.add(SkinnedMeshInverseBindposes::from(ibp_matrices));

            // 3b. Build Joints Vec<Entity>
            let mut joints_vec: Vec<Entity> = Vec::with_capacity(si.bones.len());
            let mut missing_bone = false;
            if !is_main_skeleton {
                // --- ATTACHABLE NIF LOGIC ---
                // `base_skeleton_map_holder` is `&ActiveSkeletonBones`
                if let Some(target_skeleton_id) = target_skeleton_id_opt {
                    if let Some(skeleton) = &skeleton_map.skeletons.get(&target_skeleton_id)
                    {
                        for (bone_order_idx, bone_link_opt_in_current_nif) in
                            si.bones.iter().enumerate()
                        {
                            if let Some(bone_nif_idx_in_current_nif) =
                                bone_link_opt_in_current_nif
                                {
                                    // Get the NiNode block from the *current NIF's block_map* to find its name
                                    if let Some(ParsedBlock::Node(node_data_in_current_nif)) =
                                        block_map.get(bone_nif_idx_in_current_nif)
                                        {
                                            let bone_name = node_data_in_current_nif.name();
                                            if let Some(bone_data) =
                                                skeleton.get_bone_by_name(&bone_name)
                                            {
                                                joints_vec.push(bone_data.entity);
                                            } else {
                                                warn!(
                                                    "Attachable TriShape {}: Bone '{}' (NIF idx {} in current NIF) not found in active base skeleton map.",
                                                    nif_index,
                                                    &format!("Node: {}", bone_name),
                                                    bone_nif_idx_in_current_nif
                                                );
                                                missing_bone = true;
                                                break;
                                            }
                                        } else {
                                            warn!(
                                                "Attachable TriShape {}: Bone NIF idx {} in current NIF's SkinInstance does not point to a valid NiNode.",
                                                nif_index, bone_nif_idx_in_current_nif
                                            );
                                            missing_bone = true;
                                            break;
                                        }
                                } else {
                                    warn!(
                                        "Attachable TriShape {}: Missing bone link at order index {} in SkinInstance.",
                                        nif_index, bone_order_idx
                                    );
                                    missing_bone = true;
                                    break;
                                }
                        }
                    }
                }
            } else {
                for (i, bone_link_opt) in si.bones.iter().enumerate() {
                    if let Some(bone_nif_index) = bone_link_opt {
                        // Look up the Entity spawned for this bone's NiNode index
                        if let Some(bone_entity) = entity_map.get(bone_nif_index) {
                            joints_vec.push(*bone_entity);
                        } else {
                            warn!(
                                "   Bone node link #{} (index {}) from SkinInstance {} not found in spawned entity map! Cannot add SkinnedMesh.",
                                i, bone_nif_index, si_index
                            );
                            missing_bone = true;
                            break;
                        }
                    } else {
                        warn!(
                            "   Bone node link #{} from SkinInstance {} is None! Cannot add SkinnedMesh.",
                            i, si_index
                        );
                        missing_bone = true;
                        break;
                    }
                }
            }
            // 3c. Add SkinnedMesh Component (if all bones found)
            if !missing_bone && joints_vec.len() == sd.bone_list.len() {
                commands.entity(current_entity_id).insert(SkinnedMesh {
                    inverse_bindposes: ibp_handle,
                    joints: joints_vec,
                });
            } else {
                warn!(
                    "   Failed to add SkinnedMesh component due to missing bone entity or count mismatch."
                );
            }
        } // End if skinning data found
    }

    let mut final_material: StandardMaterial = match final_texture_info {
        Some(ref tex_info) if tex_info.base_texture_path.is_some() => {
            // --- Case 1: Textures found ---
            // Get the base material properties data (or a default if no handle or asset not found)
            let base_material_data = base_material_handle
                .and_then(|h| materials.get(&h).cloned()) // Clone if found in assets
                .unwrap_or_else(|| {
                    warn!("TriShape {}: Textures found but no base MaterialProperty asset found or handle missing. Using default.", nif_index);
                    StandardMaterial::default() // Use default data
                });
            let mut textured_material = base_material_data;

            // Load and assign base texture
            if let Some(base_path) = &tex_info.base_texture_path {
                let bevy_path = resolve_nif_path(base_path); // Your path resolver
                let texture_handle: Handle<Image> = asset_server.load(&bevy_path);
                textured_material.base_color_texture = Some(texture_handle);
            }

            // Set a default alpha mode when textures are present, unless explicitly overridden
            // This prevents inheriting an unwanted alpha mode from a base material if it had one
            // but no AlphaProperty was found for *this* shape specifically.
            if final_alpha_mode.is_none() {
                textured_material.alpha_mode = AlphaMode::Opaque; // Sensible default with texture
            }

            // This overrides the default Opaque set above if an AlphaProperty was found
            if let Some(alpha_override) = final_alpha_mode {
                textured_material.alpha_mode = alpha_override;
                // Optionally set double_sided / cull_mode based on alpha mode
                if alpha_override != AlphaMode::Opaque {
                    textured_material.double_sided = true;
                    textured_material.cull_mode = None; // Disable backface culling for transparency
                } else {
                    // Reset to default if base material had these changed
                    textured_material.double_sided = false;
                    textured_material.cull_mode = Some(Face::Back);
                }
            }
            // If `final_alpha_mode` was None, the `AlphaMode::Opaque` set earlier (if base texture exists)
            // or the mode inherited from `base_material_data` remains.

            // Return the configured material struct directly
            textured_material
        }

        // --- Case 2: No textures found ---
        _ => {
            // Get base material data or use a distinct fallback struct
            let mut material_data = base_material_handle
                .and_then(|h| materials.get(&h).cloned()) // Clone if found
                .unwrap_or_else(|| {
                    warn!("TriShape {}: No Material or Texturing found. Using fallback white material.", nif_index);
                    // Distinct fallback color, otherwise default
                    StandardMaterial { base_color: Color::WHITE, ..Default::default() }
                });

            // Still apply alpha mode if found, even without textures
            if let Some(alpha_override) = final_alpha_mode {
                material_data.alpha_mode = alpha_override;
                // Optionally adjust double_sided/cull_mode here too if needed
                if alpha_override != AlphaMode::Opaque {
                    material_data.double_sided = true;
                    material_data.cull_mode = None;
                } else {
                    // Reset to default if base material had these changed
                    material_data.double_sided = false;
                    material_data.cull_mode = Some(Face::Back);
                }
            }
            // If `final_alpha_mode` is None, the alpha mode from the base material
            // (or the default `AlphaMode::Opaque` from `StandardMaterial::default()`) is kept.

            // Return the base or fallback material struct (potentially with alpha modified)
            material_data
        }
    };
    final_material.alpha_mode = AlphaMode::Mask(1.0);
    final_material.cull_mode = None;
    let final_material_handle = materials.add(final_material);

    // --- Insert Mesh and Material components ---
    commands.entity(current_entity_id).insert((
            Mesh3d(mesh_handle), // Handle<Mesh>
            MeshMaterial3d(final_material_handle),
    ));
}
_ => {
    // This block type isn't visually spawned in the hierarchy
    commands.entity(current_entity_id).despawn(); // Despawn the empty entity created
    entity_map.remove(&nif_index); // Remove from map
    should_keep_entity = false; // Don't try to parent it
}
}
if should_keep_entity {
    commands.entity(parent_entity).add_child(current_entity_id);
}
}*/
