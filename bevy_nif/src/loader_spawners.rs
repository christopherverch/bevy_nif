use bevy_asset::{Handle, LoadContext, RenderAssetUsages};
use bevy_camera::visibility::Visibility;
use bevy_ecs::component::Component;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::name::Name;
use bevy_ecs::reflect::ReflectComponent;
use bevy_ecs::{entity::Entity, world::World};
use bevy_log::{error, warn};
use bevy_material::AlphaMode;
use bevy_math::{Mat4, Quat, Vec2, Vec3};
use bevy_mesh::skinning::SkinnedMeshInverseBindposes;
use bevy_mesh::{Indices, Mesh, PrimitiveTopology};
use bevy_reflect::Reflect;
use nif::{NiKey, loader::Nif};
use nif::{NiSkinInstance, NiTriBasedGeomData, NiTriShapeData, NiType};

use crate::loader::{PendingNifSkins, SpawnContext};
use crate::skeleton::Skeleton;
use crate::spawning_ni_helpers::{process_nimaterialproperty, process_nitexturingproperty};
use crate::{NifColliderData, hash_str};

use bevy_pbr::{MeshMaterial3d, StandardMaterial};
use bevy_render::mesh::Mesh3d;
use bevy_transform::components::Transform;

#[derive(Reflect)]
pub struct NifBox {
    pub translation: Vec3,
    pub rotation: Quat,
    pub extents: Vec3,
}

pub fn spawn_nif_node_recursive(
    nif: &Nif,
    spawn_context: &mut SpawnContext,
    load_context: &mut LoadContext,
    current_key: NiKey,
    parent_entity: Entity,
    parent_bone_name_opt: Option<&str>,
    mut skeleton_opt: Option<&mut Skeleton>,
    pending_nif_skins: &mut Option<PendingNifSkins>,
    world: &mut World,
    is_nonvisual_collider: bool,
    mut create_collider_from_mesh: bool,
) {
    if spawn_context
        .already_spawned_nodes
        .contains_key(&current_key)
    {
        // Already spawned this node elsewhere (e.g., via NiSkinInstance), skip it
        return;
    }

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

            let new_ninode_entity = world
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
            if let Some(skeleton) = skeleton_opt.as_mut() {
                world
                    .entity_mut(new_ninode_entity)
                    .insert(Name::new(ni_node.name.to_string()));
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
                    load_context,
                    child.key,
                    new_ninode_entity,
                    current_bone_name_opt,
                    skeleton_opt.as_deref_mut(),
                    pending_nif_skins,
                    world,
                    is_nonvisual_collider,
                    create_collider_from_mesh,
                );
            }
        }
        NiType::NiTriShape(ni_trishape) => {
            let bevy_transform = Transform {
                translation: ni_trishape.translation,
                rotation: Quat::from_mat3(&ni_trishape.rotation),
                scale: Vec3::splat(ni_trishape.scale),
            };

            let new_nitrishape_entity = world
                .spawn((
                    Name::new(ni_trishape.name.clone()),
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
                let nif_collider_data = create_collider(trishape_data);
                world
                    .entity_mut(new_nitrishape_entity)
                    .insert(nif_collider_data);
                if is_nonvisual_collider {
                    return;
                }
            }

            let name_ref: &str = &ni_trishape.name;
            // Make shadow invisible (or if it's the main skeleton, the bones)
            if name_ref == "Tri Shadow" || name_ref == "Tri QuadPatch01" || skeleton_opt.is_some() {
                world
                    .entity_mut(new_nitrishape_entity)
                    .insert(Visibility::Hidden);
            }

            // Convert and register mesh via LoadContext instead of meshes resource store
            let mut mesh_opt = convert_nif_mesh(trishape_data);

            // Process properties (materials and textures)
            let ni_properties = &ni_trishape.properties;
            let mut material_opt: Option<StandardMaterial> = None;
            let mut texture_handle_opt = None;

            for property in ni_properties {
                if let Some(ni_property) = nif.objects.get(property.key) {
                    match ni_property {
                        NiType::NiTexturingProperty(tex_prop) => {
                            // Note: use load_context instead of asset_server inside loaders
                            texture_handle_opt =
                                process_nitexturingproperty(tex_prop, nif, load_context);
                        }
                        NiType::NiMaterialProperty(mat_prop) => {
                            material_opt = Some(process_nimaterialproperty(mat_prop));
                        }
                        _ => {}
                    }
                }
            }

            if let Some(mut material) = material_opt {
                material.base_color_texture = texture_handle_opt.take();
                material.alpha_mode = AlphaMode::Mask(1.0);

                let material_h = load_context.add_loaded_labeled_asset(
                    format!("Material_{}", &ni_trishape.name),
                    material.into(),
                );
                world
                    .entity_mut(new_nitrishape_entity)
                    .insert(MeshMaterial3d(material_h));
            }

            let skin_key = ni_trishape.skin_instance.key;
            if let Some(ni_skin_instance) = nif.objects.get(skin_key) {
                if let NiType::NiSkinInstance(skin_instance) = ni_skin_instance {
                    // Store skin/bone name reference data on the entity for runtime binding,
                    // since live Entity IDs aren't available during asset loading.
                    if skeleton_opt.is_none() {
                        apply_skin_instance(
                            nif,
                            skin_instance,
                            new_nitrishape_entity,
                            &ni_trishape.name,
                            &mut mesh_opt,
                            load_context,
                            pending_nif_skins,
                            world,
                        );
                    }
                }
            }
            let label = format!("Mesh_{}", name_ref);
            if let Some(mesh_data) = mesh_opt.take() {
                let mut mesh = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::default(),
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, mesh_data.0);

                // Create the Bevy Mesh
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, mesh_data.1); // MOVE

                // Insert UVs if they exist
                if !mesh_data.2.is_empty() {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, mesh_data.2); // MOVE
                }
                // Insert the final flat indices
                mesh.insert_indices(Indices::U16(mesh_data.3));
                let handle = load_context.add_loaded_labeled_asset(label, mesh.into());
                world
                    .entity_mut(new_nitrishape_entity)
                    .insert(Mesh3d(handle));
            }
        }

        NiType::RootCollisionNode(node) => {
            let bevy_transform = Transform {
                translation: node.translation,
                rotation: Quat::from_mat3(&node.rotation),
                scale: Vec3::splat(node.scale),
            };

            let new_collision_node_entity = world
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

            spawn_context
                .already_spawned_nodes
                .insert(current_key, new_collision_node_entity);

            for child in &node.children {
                spawn_nif_node_recursive(
                    nif,
                    spawn_context,
                    load_context,
                    child.key,
                    new_collision_node_entity,
                    None,
                    skeleton_opt.as_deref_mut(),
                    pending_nif_skins,
                    world,
                    true,
                    false,
                );
            }
        }
        _ => {}
    }
}
fn create_collider(data: &NiTriShapeData) -> NifColliderData {
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
    NifColliderData::Trimesh(vertices, u32_indices)
}
fn apply_skin_instance(
    nif: &Nif,
    skin_instance: &NiSkinInstance,
    current_entity: Entity,
    current_name: &str,
    mesh_opt: &mut Option<(Vec<Vec3>, Vec<Vec3>, Vec<Vec2>, Vec<u16>)>,
    load_context: &mut LoadContext,
    pending_nif_skins: &mut Option<PendingNifSkins>,
    world: &mut World,
) {
    let Some(mesh_ref) = mesh_opt else { return };
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
    // Only insert joint indices/weights if some other node didn't already do it for this mesh
    let vertex_count = mesh_ref.0.len();

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
    let ibp_handle = load_context.add_loaded_labeled_asset(
        format!("IBP_{}", current_name),
        SkinnedMeshInverseBindposes::from(ibp_matrices).into(),
    ); // Build Joints Vec<Entity>
    let mut bone_names = Vec::with_capacity(skin_instance.bones.len());
    for bone_link in &skin_instance.bones {
        if let Some(NiType::NiNode(bone_node)) = nif.objects.get(bone_link.key) {
            // TODO:: find some way to mem::move this
            bone_names.push(bone_node.name.clone());
        } else {
            bone_names.push(String::new());
        }
    }
    // Required for animation
    let Some(mesh) = mesh_opt.take() else {
        return;
    };

    world.entity_mut(current_entity).insert(PendingNifSkin {
        mesh: Some(mesh),
        joint_indices,
        joint_weights,
        inverse_bindposes: ibp_handle,
        bone_names,
    });
    if let Some(pending_nif_skins) = pending_nif_skins {
        pending_nif_skins.pending.push(current_entity);
    } else {
        let mut pending_skins = Vec::new();
        pending_skins.push(current_entity);
        *pending_nif_skins = Some(PendingNifSkins {
            pending: pending_skins,
            skeleton_entity: None,
        });
    }
}
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct NeedsNifSkinMapping;
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct PendingNifSkin {
    // So we can .take() out of it later
    pub mesh: Option<(Vec<Vec3>, Vec<Vec3>, Vec<Vec2>, Vec<u16>)>,
    pub joint_indices: Vec<[u16; 4]>,
    pub joint_weights: Vec<[f32; 4]>,
    pub inverse_bindposes: Handle<SkinnedMeshInverseBindposes>,
    pub bone_names: Vec<String>,
}

pub fn convert_nif_mesh(
    data: &NiTriShapeData,
) -> Option<(Vec<Vec3>, Vec<Vec3>, Vec<Vec2>, Vec<u16>)> {
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
    let final_mesh_opt;
    let flat_indices: Vec<u16> = indices.into_iter().cloned().flatten().collect();
    if !normals.is_empty() {
        final_mesh_opt = Some((vertices, normals, uvs, flat_indices));
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
) -> Option<(Vec<Vec3>, Vec<Vec3>, Vec<Vec2>, Vec<u16>)> {
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
    let mut final_vertices: Vec<Vec3> = Vec::with_capacity(new_vertex_count);
    let mut final_normals: Vec<Vec3> = Vec::with_capacity(new_vertex_count);
    let mut final_indices: Vec<u16> = Vec::with_capacity(new_vertex_count);
    // Only create UV buffer if original UVs were present
    let mut final_uvs: Option<Vec<Vec2>> =
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
        let normalized_normal_array = face_normal.try_normalize().unwrap_or(Vec3::Y);

        // Add duplicated vertex positions
        final_vertices.push(v0);
        final_vertices.push(v1);
        final_vertices.push(v2);

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
                    uvs_out.push([uvs_in[idx0].x, uvs_in[idx0].y].into());
                    uvs_out.push([uvs_in[idx1].x, uvs_in[idx1].y].into());
                    uvs_out.push([uvs_in[idx2].x, uvs_in[idx2].y].into());
                } else {
                    warn!(
                        "Missing UV data for original indices in triangle {}, using default [0,0].",
                        i
                    );
                    uvs_out.push([0.0, 0.0].into());
                    uvs_out.push([0.0, 0.0].into());
                    uvs_out.push([0.0, 0.0].into());
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

    let final_uvs = final_uvs.unwrap_or_default();

    Some((final_vertices, final_normals, final_uvs, final_indices))
}
