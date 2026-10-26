use crate::attach_parts::AttachmentType;
use crate::loader::NifNodeIndex;
use crate::nif_animation::SkeletonMap;
use crate::spawning_ni_helpers::{process_nimaterialproperty, process_nitexturingproperty};
use crate::{NeedsNifPhysics, hash_str, skeleton::*};
use avian3d::prelude::{Collider, RigidBody};
use bevy_asset::{AssetServer, Assets, Handle, RenderAssetUsages};
use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    event::EntityEvent,
    hierarchy::ChildOf,
    query::Without,
    system::{Commands, Query, Res, ResMut},
};
use bevy_log::{error, warn};
use bevy_material::AlphaMode;
use bevy_math::{Mat4, Quat, Vec2, Vec3};
use bevy_mesh::{Indices, Mesh3d, PrimitiveTopology};
use bevy_mesh::{
    Mesh, VertexAttributeValues,
    skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
};
use bevy_pbr::{MeshMaterial3d, StandardMaterial};
use bevy_transform::components::Transform;
use nif::{NiKey, NiSkinInstance, NiType, loader::Nif};
use nif::{NiTriBasedGeomData, NiTriShapeData};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Component)]
pub struct NeedsNifAnimator {
    pub handle: Handle<Nif>,
    pub skeleton_id: u64,
}
#[derive(EntityEvent, Clone, Debug)]
pub struct NifInstantiated {
    pub entity: Entity,
    pub handle: Handle<Nif>,
    pub skeleton_id_opt: Option<u64>,
}
#[derive(Component)]
pub struct LoadedNifScene(pub Entity);

#[derive(Default, Debug, Component)]
pub struct NifScene(pub Handle<Nif>);

pub fn spawn_nif_scenes(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    nif_assets: Res<Assets<Nif>>,
    asset_server: Res<AssetServer>,
    mut new_scenes: Query<
        (Entity, &NifScene, Option<&mut AttachmentType>),
        Without<LoadedNifScene>,
    >,
    mut inverse_bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut skeleton_map_res: ResMut<SkeletonMap>,
) {
    for (original_entity, nif_scene_component, attachment_type_opt) in &mut new_scenes {
        let is_skeleton = attachment_type_opt
            .as_ref()
            .map(|att| matches!(**att, AttachmentType::MainSkeleton { skeleton_id: _ }))
            .unwrap_or_default();

        let target_skeleton_id_opt = attachment_type_opt.map(|att| att.get_target_skeleton_id());
        dbg!(nif_scene_component.0.path());

        // If this asset requires a skeleton, check if the skeleton it attaches to is ready
        if !is_skeleton {
            if let Some(id) = target_skeleton_id_opt {
                if !skeleton_map_res.root_skeleton_entity_map.contains_key(&id) {
                    continue; // Skip this asset for now, try the next one
                }
            }
        }

        let Some(nif) = nif_assets.get(&nif_scene_component.0) else {
            continue;
        };

        let mut skeleton = Skeleton::new();
        let mut spawn_context = SpawnContext {
            target_skeleton_id_opt,
            is_skeleton,
            asset_server: &asset_server,
            already_spawned_nodes: HashMap::default(),
            nif_node_index: NifNodeIndex::default(),
            ninodes_with_bvs: Vec::new(),
        };

        // Spawn root nodes, parenting to original entity
        for current_node in nif.roots.iter() {
            let root_node_entity = commands
                .spawn((
                    Transform::default(),
                    Visibility::Inherited,
                    ChildOf(original_entity),
                ))
                .id();

            spawn_nif_node_recursive(
                nif,
                &mut spawn_context,
                current_node.key,
                root_node_entity,
                None,
                &mut skeleton,
                &mut skeleton_map_res,
                &mut materials,
                &mut meshes,
                &mut inverse_bindposes,
                &mut commands,
                false,
                false,
            );
        }

        if !spawn_context.ninodes_with_bvs.is_empty() {
            commands
                .entity(original_entity)
                .insert(NeedsNifPhysics(spawn_context.ninodes_with_bvs));
        }

        commands.entity(original_entity).insert((
            LoadedNifScene(original_entity),
            spawn_context.nif_node_index,
        ));

        if is_skeleton {
            let Some(target_skeleton_id) = target_skeleton_id_opt else {
                panic!("main skeleton should always have a skeleton id");
            };
            skeleton_map_res
                .root_skeleton_entity_map
                .insert(target_skeleton_id, original_entity);
            skeleton_map_res
                .skeletons
                .insert(target_skeleton_id, skeleton);

            commands.entity(original_entity).insert(NeedsNifAnimator {
                handle: nif_scene_component.0.clone(),
                skeleton_id: target_skeleton_id,
            });
        }
    }
}
struct SpawnContext<'a> {
    target_skeleton_id_opt: Option<u64>,
    is_skeleton: bool,
    asset_server: &'a AssetServer,
    /// In case of a circular dependency
    already_spawned_nodes: HashMap<NiKey, Entity>,
    nif_node_index: NifNodeIndex,
    ninodes_with_bvs: Vec<(Entity, NiKey)>,
}
fn spawn_nif_node_recursive<'a>(
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
    is_nonvisual_collider: bool,
    mut create_collider_from_mesh: bool,
) {
    if spawn_context
        .already_spawned_nodes
        .contains_key(&current_key)
    {
        // We already spawned this node somewhere else (such as NiSkinInstance), skip it
        return;
    };
    let Some(ni_type) = nif.objects.get(current_key) else {
        return;
    };
    match ni_type {
        NiType::NiNode(ni_node) => {
            let bevy_transform = Transform {
                translation: ni_node.translation,
                rotation: Quat::from_mat3(&ni_node.rotation),
                scale: Vec3::splat(ni_node.scale),
            };

            let new_ninode_entity = commands
                .spawn((
                    bevy_transform,
                    ChildOf(parent_entity),
                    Visibility::Inherited,
                ))
                .id();

            spawn_context
                .nif_node_index
                .named_nodes
                .insert(hash_str(&ni_node.name), new_ninode_entity);

            if let Some(_bounding_volume) = &ni_node.bounding_volume {
                // dbg!("pushing bv to", new_ninode_entity);
                spawn_context
                    .ninodes_with_bvs
                    .push((new_ninode_entity, current_key));
            }

            spawn_context
                .already_spawned_nodes
                .insert(current_key, new_ninode_entity);

            let mut current_bone_name_opt = None;
            if spawn_context.is_skeleton {
                skeleton.add_bone(
                    new_ninode_entity,
                    ni_node.name.to_string(), // Use the raw NIF name
                    parent_bone_name_opt,
                );
                current_bone_name_opt = Some(ni_node.name.as_str());
            }
            create_collider_from_mesh = ni_node.mesh_collision();
            for child in &ni_node.children {
                spawn_nif_node_recursive(
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
                    is_nonvisual_collider,
                    create_collider_from_mesh,
                );
            }
        }
        NiType::NiTriShape(ni_trishape) => {
            let nif_transform = ni_trishape;
            let bevy_transform = Transform {
                translation: ni_trishape.translation,
                rotation: Quat::from_mat3(&nif_transform.rotation),
                scale: Vec3::splat(ni_trishape.scale),
            };
            // Create the entity and set up the name
            let new_nitrishape_entity = commands
                .spawn((
                    bevy_transform,
                    ChildOf(parent_entity),
                    Visibility::Inherited,
                ))
                .id();
            let Some(geom_data_type) = nif.objects.get(ni_trishape.geometry_data.key) else {
                warn!("NiTriShape missing mesh!");
                return;
            };
            let trishape_data = match geom_data_type {
                NiType::NiTriShapeData(trishape_data) => trishape_data,
                _ => return,
            };

            spawn_context
                .ninodes_with_bvs
                .push((new_nitrishape_entity, current_key));

            spawn_context
                .nif_node_index
                .tri_shapes
                .push(new_nitrishape_entity);
            if is_nonvisual_collider || create_collider_from_mesh {
                let collider = create_collider(trishape_data);
                commands
                    .entity(new_nitrishape_entity)
                    .insert((RigidBody::Static, collider));
                if is_nonvisual_collider {
                    return;
                }
            }
            let name_ref: &str = &ni_trishape.name;
            // Make shadow invisible (or if it's the main skeleton, the bones)
            if name_ref == "Tri Shadow"
                || name_ref == "Tri QuadPatch01"
                || spawn_context.is_skeleton
            {
                commands
                    .entity(new_nitrishape_entity)
                    .insert(Visibility::Hidden);
            }

            let mesh_handle = {
                let Some(mesh) = convert_nif_mesh(trishape_data) else {
                    return;
                };
                let mesh_handle = meshes.add(mesh);
                commands
                    .entity(new_nitrishape_entity)
                    .insert(Mesh3d(mesh_handle.clone()));
                /*
                use bevy_gizmos::aabb::ShowAabbGizmo;
                commands
                    .entity(new_nitrishape_entity)
                    .insert(ShowAabbGizmo {
                        color: Some(Color::WHITE),
                    });
                */
                mesh_handle
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
                // Is this necessary?
                material.alpha_mode = AlphaMode::Mask(1.0);
                let material_h = materials.add(material);
                commands
                    .entity(new_nitrishape_entity)
                    .insert(MeshMaterial3d(material_h));
            }

            let skin_key = ni_trishape.skin_instance.key;
            if let Some(ni_skin_instance) = nif.objects.get(skin_key) {
                match ni_skin_instance {
                    NiType::NiSkinInstance(skin_instance) => {
                        apply_skin_instance(
                            nif,
                            spawn_context,
                            skin_instance,
                            new_nitrishape_entity,
                            skeleton_map,
                            &mesh_handle,
                            meshes,
                            inverse_bindposes,
                            commands,
                        );
                    }
                    _ => {}
                }
            }
        }

        NiType::RootCollisionNode(node) => {
            let bevy_transform = Transform {
                translation: node.translation,
                rotation: Quat::from_mat3(&node.rotation),
                scale: Vec3::splat(node.scale),
            };
            let new_collision_node_entity = commands
                .spawn((
                    bevy_transform,
                    ChildOf(parent_entity),
                    Visibility::Inherited,
                ))
                .id();
            spawn_context
                .nif_node_index
                .named_nodes
                .insert(hash_str(&node.name), new_collision_node_entity);
            // dbg!("pushing bv to", new_collision_node_entity);

            spawn_context
                .already_spawned_nodes
                .insert(current_key, new_collision_node_entity);
            for child in &node.children {
                spawn_nif_node_recursive(
                    nif,
                    spawn_context,
                    child.key,
                    new_collision_node_entity,
                    None,
                    skeleton,
                    skeleton_map,
                    materials,
                    meshes,
                    inverse_bindposes,
                    commands,
                    true,
                    false,
                );
            }
        }
        _ => {}
    }
}

/// Apply Skinning Attributes
fn apply_skin_instance(
    nif: &Nif,
    spawn_context: &mut SpawnContext,
    skin_instance: &NiSkinInstance,
    current_entity: Entity,
    skeleton_map: &mut ResMut<SkeletonMap>,
    mesh_handle: &Handle<Mesh>,
    meshes: &mut ResMut<Assets<Mesh>>,
    inverse_bindposes: &mut ResMut<Assets<SkinnedMeshInverseBindposes>>,
    commands: &mut Commands,
) {
    if spawn_context.is_skeleton {
        return;
    }
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
    //  Add vertex attributes
    let Some(mut mesh) = meshes.get_mut(mesh_handle) else {
        warn!(
            "   Could not get mutable mesh asset for handle {:?}",
            mesh_handle.id()
        );
        return;
    };

    // Only insert joint indices/weights if some other node didn't already do it for this mesh
    if mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX).is_some() {
        return;
    }
    let Some(vertex_count) = mesh.attribute(Mesh::ATTRIBUTE_POSITION).map(|a| a.len()) else {
        warn!(
            "   Could not apply skinning attributes: Mesh for {:?} missing positions?",
            skin_instance.root.key,
        );
        return;
    };

    // The 4 indices of the bones that affect this vertex, like [bone1_idx, bone2_idx,..]
    let mut joint_indices: Vec<[u16; 4]> = vec![[0, 0, 0, 0]; vertex_count];
    // How much each bone in the above list pulls this vertex, should sum to 1.0
    let mut joint_weights: Vec<[f32; 4]> = vec![[0.0, 0.0, 0.0, 0.0]; vertex_count];
    let mut vertex_bone_counts: Vec<u8> = vec![0; vertex_count];

    // loop through the bone data and populate indices and vertex weights
    for (bone_list_idx, bone_data) in skin_data.bone_data.iter().enumerate() {
        // Gpus don't like when there are more than 256 bones
        if bone_list_idx >= 256 {
            warn!("Too many bones in bone_data, skipping any past 256");
            continue;
        }
        for (index, weight) in &bone_data.vertex_weights {
            let vertex_index = *index as usize;
            if let Some(slot) = vertex_bone_counts.get_mut(vertex_index) {
                // Don't populate more than 4 vertex weights if the nif data is broken
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
    // Normalize weights
    for weights in joint_weights.iter_mut() {
        let sum: f32 = weights.iter().sum();
        if sum > 1e-6 {
            let inv_sum = 1.0 / sum;
            for w in weights.iter_mut() {
                *w *= inv_sum;
            }
        }
    }
    // Create Inverse Bind Pose Asset
    let mut ibp_matrices = Vec::with_capacity(skin_data.bone_data.len());
    for bone_data in &skin_data.bone_data {
        let bone_mat = Mat4::from_scale_rotation_translation(
            Vec3::splat(bone_data.scale),
            Quat::from_mat3(&bone_data.rotation),
            bone_data.translation,
        );

        // Convert NIF transform -> Bevy Transform
        // Assumes bone_data.bone_transform is the inverse bind pose

        ibp_matrices.push(bone_mat);
    }
    // Create and add the asset
    let ibp_handle = inverse_bindposes.add(SkinnedMeshInverseBindposes::from(ibp_matrices));

    // Build Joints Vec<Entity>
    let mut joints_vec: Vec<Entity> = Vec::with_capacity(skin_instance.bones.len());
    let mut missing_bone = false;
    if let Some(target_skeleton_id) = spawn_context.target_skeleton_id_opt {
        let Some(skeleton) = &skeleton_map.skeletons.get(&target_skeleton_id) else {
            error!(
                "target skeleton was not set up before nif that attaches to it! id:{}",
                target_skeleton_id
            );
            return;
        };
        for (bone_order_idx, bone_link_in_current_nif) in skin_instance.bones.iter().enumerate() {
            let Some(bone_object_nitype) = nif.objects.get(bone_link_in_current_nif.key) else {
                warn!(
                    "Attachable TriShape root {:?}: Missing bone link at order index {} in SkinInstance.",
                    skin_instance.root, bone_order_idx
                );
                missing_bone = true;
                break;
            };
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
        }
    } else {
        warn!("Missing target skeleton for skin instance!",);
    }
    // 3c. Add Bevy SkinnedMesh Component (if all bones found)
    // Required for animation
    if !missing_bone && joints_vec.len() == skin_data.bone_data.len() {
        // Insert Bevy vertex attributes
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_JOINT_INDEX,
            VertexAttributeValues::Uint16x4(joint_indices),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_JOINT_WEIGHT,
            VertexAttributeValues::Float32x4(joint_weights),
        );
        commands.entity(current_entity).insert(SkinnedMesh {
            inverse_bindposes: ibp_handle,
            joints: joints_vec,
        });
    } else {
        if missing_bone {
            warn!("Failed to add SkinnedMesh component due to missing bone entity");
        }
        if joints_vec.len() != skin_data.bone_data.len() {
            warn!("Failed to add SkinnedMesh component due to count mismatch.");
        }
    }
}
pub fn convert_nif_mesh(data: &NiTriShapeData) -> Option<Mesh> {
    // TODO:: not sure what to do with shared normals
    let NiTriShapeData {
        base,
        triangles,
        shared_normals: _shared_normals,
    } = data;
    let NiTriBasedGeomData { base } = base;
    let vertices = base.vertices.clone();
    let normals = base.normals.clone();
    let uvs = base.uv_sets.clone();
    let indices = triangles;
    let final_mesh_opt: Option<Mesh>;
    let flat_indices: Vec<u16> = indices.into_iter().cloned().flatten().collect();
    if !normals.is_empty() {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices);

        // Create the Bevy Mesh
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals); // MOVE

        // Insert UVs if they exist
        if !uvs.is_empty() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs); // MOVE
        }
        // Insert the final flat indices
        mesh.insert_indices(Indices::U16(flat_indices));

        final_mesh_opt = Some(mesh);
    } else {
        // Fallback: Need to call the helper function, which needs to be updated
        // to accept the Bevy array types (Vec<[f32; 3]>).

        // We pass references to the moved data's components before they are consumed below.
        final_mesh_opt = create_mesh_with_flat_normals(
            // Pass the data that contains the vertex position info
            vertices,
            flat_indices,
            if uvs.is_empty() { None } else { Some(&uvs) },
        );
    }

    // TODO: Add vertex colors (Mesh::ATTRIBUTE_COLOR) if data.has_vertex_colors is true

    if let Some(mesh) = final_mesh_opt {
        Some(mesh)
    } else {
        None
    }
}
fn create_mesh_with_flat_normals(
    original_vertices_nif: Vec<Vec3>,
    original_indices: Vec<u16>,
    original_uvs: Option<&Vec<Vec2>>,
) -> Option<Mesh> {
    let vertex_count = original_vertices_nif.len();
    if vertex_count == 0 {
        warn!("Cannot compute flat normals: No vertices provided.");
        return None;
    }
    if original_indices.is_empty() {
        warn!("Cannot compute flat normals: No indices provided.");
        return None;
    }
    if original_indices.len() % 3 != 0 {
        warn!(
            "Cannot compute flat normals: Index count ({}) is not a multiple of 3.",
            original_indices.len()
        );
        return None;
    }

    let num_triangles = original_indices.len() / 3;
    let new_vertex_count = num_triangles * 3;

    // Initialize buffers for the new mesh data
    let mut final_vertices: Vec<[f32; 3]> = Vec::with_capacity(new_vertex_count);
    let mut final_normals: Vec<[f32; 3]> = Vec::with_capacity(new_vertex_count);
    let mut final_indices: Vec<u16> = Vec::with_capacity(new_vertex_count);
    // Only create UV buffer if original UVs were present
    let mut final_uvs: Option<Vec<[f32; 2]>> =
        original_uvs.map(|_| Vec::with_capacity(new_vertex_count));

    for i in 0..num_triangles {
        // Get original vertex indices for this triangle
        let mut idx0 = original_indices[i * 3] as usize;
        let mut idx1 = original_indices[i * 3 + 1] as usize;
        let mut idx2 = original_indices[i * 3 + 2] as usize;

        // --- Bounds Checking ---
        #[cfg(debug_assertions)]
        if idx0 >= vertex_count || idx1 >= vertex_count || idx2 >= vertex_count {
            warn!(
                "Skipping triangle {} due to out-of-bounds index (Indices: {}, {}, {}; Vertex Count: {}).",
                i, idx0, idx1, idx2, vertex_count
            );
            // Just push junk indices so the rest of the data isn't corrupted.
            idx0 = 0;
            idx1 = 0;
            idx2 = 0;
        }

        // Get vertex positions
        let v0 = original_vertices_nif[idx0];
        let v1 = original_vertices_nif[idx1];
        let v2 = original_vertices_nif[idx2];

        // Calculate face normal
        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        let face_normal = edge1.cross(edge2);

        // Normalize, handling degenerate triangles (use Y-up as default)
        let normalized_normal_array = face_normal.try_normalize().unwrap_or(Vec3::Y).to_array();

        // Add duplicated vertex positions
        final_vertices.push(v0.to_array());
        final_vertices.push(v1.to_array());
        final_vertices.push(v2.to_array());

        // Add the same face normal for all 3 vertices of this triangle
        final_normals.push(normalized_normal_array);
        final_normals.push(normalized_normal_array);
        final_normals.push(normalized_normal_array);

        // Add sequential indices for the new vertices
        let base_index = (i * 3) as u16; // Assuming fewer than 65536 triangles per original mesh part
        final_indices.push(base_index);
        final_indices.push(base_index + 1);
        final_indices.push(base_index + 2);

        // Duplicate UVs if they exist
        if let Some(ref mut uvs_out) = final_uvs {
            if let Some(uvs_in) = original_uvs {
                // Check bounds for original UVs as well
                if idx0 < uvs_in.len() && idx1 < uvs_in.len() && idx2 < uvs_in.len() {
                    uvs_out.push([uvs_in[idx0].x, uvs_in[idx0].y]);
                    uvs_out.push([uvs_in[idx1].x, uvs_in[idx1].y]);
                    uvs_out.push([uvs_in[idx2].x, uvs_in[idx2].y]);
                } else {
                    warn!(
                        "Missing UV data for original indices in triangle {}, using default [0,0].",
                        i
                    );
                    uvs_out.push([0.0, 0.0]);
                    uvs_out.push([0.0, 0.0]);
                    uvs_out.push([0.0, 0.0]);
                }
            }
        }
    }

    // sanity check
    if final_vertices.len() != final_normals.len() {
        error!(
            "Manual flat normal calculation resulted in mismatch! Verts: {}, Norms: {}",
            final_vertices.len(),
            final_normals.len()
        );
        // This shouldn't happen with this logic
        return None;
    }
    if let Some(ref uvs) = final_uvs {
        if final_vertices.len() != uvs.len() {
            error!(
                "Manual flat normal calculation resulted in UV mismatch! Verts: {}, UVs: {}",
                final_vertices.len(),
                uvs.len()
            );
            // TODO:: push default uvs until we have enough?
        }
    }

    // Create the Bevy Mesh using the generated data
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, final_vertices) // Use new vertices
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, final_normals); // Use new normals

    if let Some(final_uvs_vec) = final_uvs {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, final_uvs_vec);
    }

    mesh.insert_indices(Indices::U16(final_indices)); // Use new sequential indices

    Some(mesh)
}
fn create_collider(data: &NiTriShapeData) -> Collider {
    // TODO:: not sure what to do with shared normals
    let NiTriShapeData {
        base,
        triangles,
        shared_normals: _shared_normals,
    } = data;
    let NiTriBasedGeomData { base } = base;
    let vertices = base.vertices.clone();
    let indices = triangles;
    let u32_indices: Vec<[u32; 3]> = indices
        .iter()
        .map(|&[x, y, z]| [x as u32, y as u32, z as u32])
        .collect();
    Collider::try_trimesh(vertices, u32_indices).unwrap()
}
