use crate::animation::NiSkinData;
use crate::animation::NiSkinInstance;
// src/nif/spawner.rs
use crate::LoadedNifScene;
use crate::NifInstantiated;
use crate::nif::helper_funcs::{convert_nif_transform, resolve_nif_path};
use crate::nif::types::*;
use bevy::asset::{Assets, Handle};
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy::render::mesh::Mesh;
use bevy::render::mesh::VertexAttributeValues;
use bevy::render::mesh::skinning::SkinnedMesh;
use bevy::render::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::render::render_resource::Face;
use std::collections::HashMap;

use super::loader::Nif;
#[derive(Component)]
pub struct NifScene(pub Handle<Nif>);
#[allow(dead_code)]
pub fn spawn_nif_scenes(
    // Other needed Bevy resources and queries
    mut commands: Commands,
    nif_assets: Res<Assets<Nif>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    new_scenes: Query<(Entity, &NifScene), Without<LoadedNifScene>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut inverse_bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
) {
    for (entity, nif_scene_component) in new_scenes.iter() {
        println!("entity: {}", entity);
        // **Check if the asset for THIS entity is loaded NOW**
        // This uses Assets::get, polling the current state of loaded assets
        let Some(nif) = nif_assets.get(&nif_scene_component.0) else {
            return;
        };
        let asset_handle = &nif_scene_component.0;
        // --- Data Conversion and Spawning ---
        //pass 2
        let mut entity_map: HashMap<usize, Entity> = HashMap::new();

        // Spawn the top-level scene root entity (optional, but good practice)
        let scene_root_entity = commands
            .spawn((
                Transform {
                    translation: Vec3::new(0.0, 0.0, 0.0),
                    rotation: Quat::IDENTITY,
                    scale: Vec3::new(0.03, 0.03, 0.03),
                },
                InheritedVisibility::VISIBLE,
                Name::new(format!("NifScene_{:?}", asset_handle.id())),
            ))
            .id();
        commands.entity(entity).add_child(scene_root_entity);

        // Find the root nodes of the NIF graph. Often index 0, but could be others.
        // A simple approach is to assume index 0 is the main root.
        // A more robust approach would find nodes not listed as children of any other node.
        let nif_root_index = 0; // Assuming block 0 is the root NiNode
        let mut block_map: HashMap<usize, &ParsedBlock> = HashMap::new();
        for (index, block) in nif.raw_parsed.blocks.iter().enumerate() {
            block_map.insert(index, block);
        }

        // Start the recursive spawning process
        if let Some(root_block_data) = block_map.get(&nif_root_index) {
            // *** Pass the looked-up data (&ParsedBlock) to the first call ***
            spawn_nif_node_recursive(
                &mut commands,
                nif_root_index,
                scene_root_entity,
                &block_map,
                &mut entity_map,
                &nif.mesh_handles,
                &nif.material_handles,
                &nif.texture_info_map,
                &mut materials,
                &asset_server,
                &mut meshes,
                &mut inverse_bindposes,
                root_block_data,
            );
        } else {
            warn!("NIF root index {} not found in block_map!", nif_root_index);
        }
        info!("<<< Finished Spawning NIF entities >>>");
        commands
            .entity(entity)
            .insert(LoadedNifScene(asset_handle.clone()));
        commands.trigger(NifInstantiated(asset_handle.clone()));
    }
}
fn spawn_nif_node_recursive(
    commands: &mut Commands,
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
) {
    if entity_map.contains_key(&nif_index) {
        return;
    } // Avoid cycles/duplicates

    let bevy_transform = match block {
        ParsedBlock::Node(data) => convert_nif_transform(&data.av_base.transform),
        ParsedBlock::TriShape(data) => convert_nif_transform(&data.av_base.transform),
        _ => Transform::IDENTITY,
    };

    // Spawn using user's preferred component style
    let current_entity_id = commands
        .spawn((
            bevy_transform,
            Visibility::Visible, // Keep basic visibility
            InheritedVisibility::VISIBLE,
            Name::new(format!("NifBlock_{}", nif_index)), // Initial name
        ))
        .id();
    entity_map.insert(nif_index, current_entity_id); // Track entity immediately

    let mut should_keep_entity = true;
    match block {
        ParsedBlock::Node(node_data) => {
            // Insert Name using commands.entity()
            commands
                .entity(current_entity_id)
                .insert(Name::new(format!("NiNode: {}", node_data.name())));

            // Recurse for children
            for child_link in &node_data.children {
                if let Some(child_index) = child_link {
                    // Get child block data BEFORE recursing
                    if let Some(child_block_data) = block_map.get(child_index) {
                        spawn_nif_node_recursive(
                            commands,
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
                    println!("entered nitrishape1");
                    match block_map.get(prop_idx) {
                        Some(ParsedBlock::MaterialProperty(_))
                            if base_material_handle.is_none() =>
                        {
                            println!("no data3");
                            base_material_handle = material_handles.get(prop_idx).cloned();
                        }
                        Some(ParsedBlock::TexturingProperty(_)) if final_texture_info.is_none() => {
                            println!("no data2");
                            final_texture_info = texture_info_map.get(prop_idx).cloned();
                        }
                        Some(ParsedBlock::AlphaProperty(alpha_prop)) => {
                            println!("no data1");
                            info!("  TriShape {}: Found AlphaProperty {}", nif_index, prop_idx);
                            let flags = alpha_prop.flags;
                            let threshold = alpha_prop.threshold;
                            println!("flags and threshhold: {} {}", flags, threshold);
                            // Check flags according to common NIF specs:
                            let enable_testing = (flags & 0x0200) != 0; // Bit 9 usually enables testing
                            let enable_blending = (flags & 0x0001) != 0; // Bit 0 usually enables blending

                            if enable_testing {
                                // Use Mask mode if testing is enabled
                                let mask_cutoff = threshold as f32 / 255.0;
                                final_alpha_mode = Some(AlphaMode::Mask(mask_cutoff));
                                info!("  -> Setting AlphaMode::Mask({})", mask_cutoff);
                                // Alpha-masked materials often benefit from being double-sided
                                // You might want to handle this later when applying the final material
                                // e.g., set `final_material.double_sided = true;`
                            } else if enable_blending {
                                // Use Blend mode if blending is enabled (and testing is not)
                                final_alpha_mode = Some(AlphaMode::Blend);
                                info!("  -> Setting AlphaMode::Blend");
                                // Blended materials often benefit from being double-sided
                                // e.g., set `final_material.double_sided = true;`
                            } else {
                                // Otherwise, it's opaque
                                final_alpha_mode = Some(AlphaMode::Opaque);
                                info!("  -> Setting AlphaMode::Opaque");
                            }
                        }
                        _ => {
                            println!("no data");
                        }
                    }
                }
            }
            println!("entered nitrishape2");
            let name_ref: &str = trishape_data.av_base.net_base.name();
            let formatted_name = format!("NiTriShape: {:?}", name_ref);
            commands
                .entity(current_entity_id)
                .insert(Name::new(formatted_name));

            // --- Get Mesh ---
            let Some(data_link) = trishape_data.data_link else {
                warn!("TriShape {} missing data link.", nif_index);
                commands.entity(current_entity_id).despawn();
                entity_map.remove(&nif_index);
                return;
            };
            println!("entered nitrishape3");
            let Some(mesh_handle) = mesh_handles.get(&data_link).cloned() else {
                warn!(
                    "Mesh handle not found for data link {} from TriShape {}.",
                    data_link, nif_index
                );
                commands.entity(current_entity_id).despawn();
                entity_map.remove(&nif_index);
                return;
            };
            println!("entered nitrishape4");
            info!(
                "   Found Mesh Handle {:?} using data_link {} for TriShape {}",
                mesh_handle.id(),
                data_link,
                nif_index
            );

            // --- Find Associated Skinning Data ---
            let mut skin_instance_data: Option<&NiSkinInstance> = None;
            let mut skin_data: Option<&NiSkinData> = None;
            let mut skin_instance_block_index: Option<usize> = None;

            // Find linked NiSkinInstance (adjust this logic if link isn't controller_link)
            let controller_link = trishape_data.av_base.net_base.controller_link;
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
                info!(
                    "TriShape {} has no controller link for potential skin.",
                    nif_index
                );
            }

            println!("entered nitrishape5");
            // --- Apply Skinning Attributes & Spawn Skeleton (if needed) ---
            if let (Some(sd), Some(si), Some(si_index)) =
                (skin_data, skin_instance_data, skin_instance_block_index)
            {
                // 1. Add vertex attributes (Your existing, correct logic)
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
                        info!(
                            "   -> Inserted JOINT_INDEX and JOINT_WEIGHT attributes for TriShape {}.",
                            nif_index
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

                println!("entered nitrishape8");
                // --- *** START: ADDED Step 2: Ensure Skeleton is Spawned *** ---
                // 2. Spawn Skeleton Hierarchy
                let mut skeleton_ready = false;
                if let Some(skeleton_root_index) = si.skeleton_root {
                    println!("entered nitrishape9");
                    if !entity_map.contains_key(&skeleton_root_index) {
                        println!("entered nitrishape10");
                        if let Some(root_block_data) = block_map.get(&skeleton_root_index) {
                            spawn_nif_node_recursive(
                                commands,
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
                            );
                            println!("entered nitrishape6");
                            skeleton_ready = true;
                        } else {
                            warn!("Skeleton root link invalid");
                        }
                    } else {
                        skeleton_ready = true;
                        info!("Skeleton root already processed");
                    }
                } else {
                    warn!("SkinInstance has no skeleton root link");
                } // --- *** END: ADDED Step 2 *** ---

                if skeleton_ready {
                    println!("entered nitrishape7");
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
                    info!(
                        "   Created SkinnedMeshInverseBindposes asset {:?} with {} bones.",
                        ibp_handle.id(),
                        sd.bone_list.len()
                    );

                    // 3b. Build Joints Vec<Entity>
                    let mut joints_vec: Vec<Entity> = Vec::with_capacity(si.bones.len());
                    let mut missing_bone = false;
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

                    // 3c. Add SkinnedMesh Component (if all bones found)
                    if !missing_bone && joints_vec.len() == sd.bone_list.len() {
                        commands.entity(current_entity_id).insert(SkinnedMesh {
                            inverse_bindposes: ibp_handle,
                            joints: joints_vec,
                        });
                        info!(
                            "   Added SkinnedMesh component linking to {} joints.",
                            si.bones.len()
                        );
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
                        info!(
                            "  TriShape {}: Applying Base Texture '{}' ({})",
                            nif_index, base_path, bevy_path
                        );
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
            final_material.alpha_mode = AlphaMode::Mask(0.5);
            final_material.cull_mode = Some(Face::Back);
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
        info!(
            "Parented block {} ({:?}) to parent entity {:?}",
            nif_index, current_entity_id, parent_entity
        );
    }
}
