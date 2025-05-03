// src/nif/spawner.rs

use crate::nif::material::ColorKeyExtension; // Access extension
use crate::nif::mesh_conversion::convert_nif_mesh; // Access helper
use crate::nif::structs::*; // Access all structs via nif::structs
use crate::nif::transform_conversion::convert_nif_transform; // Access helper
use crate::setup::State; // Access State from setup module (needs pub struct State in setup.rs)

use bevy::asset::{AssetEvent, Assets, Handle};
use bevy::color::LinearRgba;
use bevy::ecs::system::{Commands, EventReader, Query, Res, ResMut};
use bevy::pbr::{AlphaMode, ExtendedMaterial, Mesh3d, MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy::render::mesh::Mesh;
use bevy::render::render_resource::Face;
use bevy::render::texture::Image;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
use std::path::PathBuf; // Keep if used in transform helper

pub fn spawn_nif_scenes_observer(
    mut events: EventReader<AssetEvent<ParsedNifData>>,
    // Other needed Bevy resources and queries
    mut commands: Commands,
    nif_assets: Res<Assets<ParsedNifData>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    q_existing_scenes: Query<(Entity, &NifSceneRoot)>,
    mut materials_colorkey: ResMut<Assets<ExtendedMaterial<StandardMaterial, ColorKeyExtension>>>,
) {
    for event in events.read() {
        let asset_handle: Handle<ParsedNifData> = match event {
            // Check the specific event variants we care about for spawning
            AssetEvent::LoadedWithDependencies { id } => Handle::Weak(*id),
            _ => {
                println!("asset event: {:?}", event);
                return;
            } // Ignore Modified, Removed, Unused etc. for spawning logic
        };

        println!("TRIGGERED");
        // Check if we already spawned a scene for this handle
        let already_spawned = q_existing_scenes
            .iter()
            .any(|(_, root)| root.source_asset == asset_handle);
        if already_spawned {
            println!(
                "Scene for {:?} already spawned, skipping.",
                asset_handle.id()
            );
            return;
        }
        if let Some(nif_data) = nif_assets.get(&asset_handle) {
            info!(
                "Observer triggered: Spawning scene for NIF asset: {:?}",
                asset_handle.id()
            );

            // --- Data Conversion and Spawning ---
            let mut block_map: HashMap<usize, &ParsedBlock> = HashMap::new();
            for (index, block) in nif_data.blocks.iter().enumerate() {
                block_map.insert(index, block);
            }
            let mut mesh_handles: HashMap<usize, Handle<Mesh>> = HashMap::new();
            let mut material_handles: HashMap<usize, Handle<StandardMaterial>> = HashMap::new();
            let mut texture_info_map: HashMap<usize, NifTextureInfo> = HashMap::new();
            // --- Pass 1: Create Bevy Assets ---
            for (index, block) in nif_data.blocks.iter().enumerate() {
                match block {
                    ParsedBlock::TriShapeData(data) => {
                        if let Some(mesh) = convert_nif_mesh(data) {
                            mesh_handles.insert(index, meshes.add(mesh));
                        }
                    }
                    ParsedBlock::MaterialProperty(mat_prop) => {
                        let mut material = StandardMaterial::default();
                        material.base_color = Color::srgb(
                            mat_prop.diffuse_color.0[0],
                            mat_prop.diffuse_color.0[1],
                            mat_prop.diffuse_color.0[2],
                        );
                        material.emissive = LinearRgba::rgb(
                            mat_prop.emissive_color.0[0],
                            mat_prop.emissive_color.0[1],
                            mat_prop.emissive_color.0[2],
                        );
                        material.metallic = 0.1;
                        material.perceptual_roughness =
                            1.0 - (mat_prop.glossiness / 100.0).clamp(0.0, 1.0);
                        material.alpha_mode = if mat_prop.alpha < 0.99 {
                            AlphaMode::Blend
                        } else {
                            AlphaMode::Opaque
                        };
                        material_handles.insert(index, materials.add(material));
                    }
                    ParsedBlock::TexturingProperty(tex_prop) => {
                        // This is where we resolve the texture path using the linked NiSourceTexture
                        let mut tex_info = NifTextureInfo::default();

                        // --- Minimal logic for Base Texture ---
                        if let Some(base_tex_data) = &tex_prop.base_texture {
                            if base_tex_data.has_texture {
                                // Check if slot is used
                                if let Some(link_idx) = base_tex_data.source_texture {
                                    // Check if link exists
                                    // Look up the linked block using the index
                                    if let Some(linked_block) = block_map.get(&link_idx) {
                                        // Check if the linked block is indeed an NiSourceTexture
                                        if let ParsedBlock::SourceTexture(src_tex) = linked_block {
                                            // Check if it points to an external file
                                            if src_tex.use_external {
                                                // Get the filename Option<String>
                                                if let Some(filename) = &src_tex.file_name {
                                                    // SUCCESS: We found the filename!
                                                    info!(
                                                        "  TexProp {}: Found Base Texture path: {}",
                                                        index, filename
                                                    );
                                                    tex_info.base_texture_path =
                                                        Some(filename.clone());
                                                    // Also store the UV set index associated with this slot
                                                    tex_info.base_uv_set = base_tex_data.uv_set;
                                                } else {
                                                    warn!(
                                                        "  TexProp {}: SourceTexture {} is external but has no filename.",
                                                        index, link_idx
                                                    );
                                                }
                                            } else {
                                                // Handle internal NiPixelData if necessary (usually not for Morrowind)
                                                warn!(
                                                    "  TexProp {}: SourceTexture {} uses internal NiPixelData (unsupported).",
                                                    index, link_idx
                                                );
                                            }
                                        } else {
                                            warn!(
                                                "  TexProp {}: Link {} does not point to an NiSourceTexture block.",
                                                index, link_idx
                                            );
                                        }
                                    } else {
                                        warn!(
                                            "  TexProp {}: Link {} is invalid (points nowhere).",
                                            index, link_idx
                                        );
                                    }
                                } else {
                                    warn!(
                                        "  TexProp {}: BaseTexture enabled but has no SourceTexture link.",
                                        index
                                    );
                                }
                                texture_info_map.insert(index, tex_info);
                            }
                        }
                    }
                    _ => {}
                }
            }
            //pass 2
            let mut entity_map: HashMap<usize, Entity> = HashMap::new();

            // Spawn the top-level scene root entity (optional, but good practice)
            let scene_root_entity = commands
                .spawn((
                    Transform {
                        translation: Vec3::new(0.0, 0.5, 0.0),
                        rotation: Quat::IDENTITY,
                        scale: Vec3::new(0.03, 0.03, 0.03),
                    },
                    InheritedVisibility::VISIBLE,
                    NifSceneRoot {
                        // Your component to mark the root
                        source_asset: asset_handle.clone(),
                    },
                    Name::new(format!("NifScene_{:?}", asset_handle.id())),
                ))
                .id();

            // Find the root nodes of the NIF graph. Often index 0, but could be others.
            // A simple approach is to assume index 0 is the main root.
            // A more robust approach would find nodes not listed as children of any other node.
            let nif_root_index = 0; // Assuming block 0 is the root NiNode

            // Start the recursive spawning process
            spawn_nif_node_recursive(
                &mut commands,
                nif_root_index,    // Start with the NIF root node index
                scene_root_entity, // Parent it to our overall scene root
                &block_map,        // Pass the parsed NIF data
                &mut entity_map,   // Map to track spawned entities
                &mesh_handles,     // Pass the asset handle maps
                &material_handles, // Pass the asset handle maps
                &texture_info_map,
                &mut materials, // Need Assets<StandardMaterial> for default/fallback
                &asset_server,  // Needed if loading textures on the fly
                &mut materials_colorkey,
            );
            info!("<<< Finished Spawning NIF entities >>>");
        }
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
    materials_colorkey: &mut ResMut<Assets<ExtendedMaterial<StandardMaterial, ColorKeyExtension>>>,
) {
    if entity_map.contains_key(&nif_index) {
        return;
    } // Avoid cycles/duplicates

    let Some(block) = block_map.get(&nif_index) else {
        warn!(
            "Recursive spawn requested for invalid nif_index: {}",
            nif_index
        );
        return;
    };
    let bevy_transform = match block {
        ParsedBlock::Node(data) => convert_nif_transform(&data.av_base.transform),
        ParsedBlock::TriShape(data) => convert_nif_transform(&data.av_base.transform),
        _ => Transform::IDENTITY,
    };

    // Spawn using user's preferred component style
    let mut entity_commands = commands.spawn((
        bevy_transform,
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        Name::new(format!("NifBlock_{}", nif_index)),
    ));
    let current_entity_id = entity_commands.id();
    entity_map.insert(nif_index, current_entity_id);
    let mut should_keep_entity = true;
    let mut texture_was_bmp = false;
    match block {
        ParsedBlock::Node(node_data) => {
            entity_commands.insert(Name::new(format!("NiNode: {}", node_data.name())));
            // Recurse for children
            for child_link in &node_data.children {
                if let Some(child_index) = child_link {
                    spawn_nif_node_recursive(
                        commands,
                        *child_index,
                        current_entity_id,
                        block_map,
                        entity_map,
                        mesh_handles,
                        material_handles,
                        texture_info_map,
                        materials,
                        asset_server,
                        materials_colorkey,
                    );
                }
            }
        }
        ParsedBlock::TriShape(trishape_data) => {
            let name_ref: &str = trishape_data.av_base.net_base.name();

            // 2. Create the formatted String using the name reference
            let formatted_name = format!("NiTriShape: {:?}", name_ref);

            // 3. Insert the Name component using the created String
            //    This line should now work without the trait bound error.
            entity_commands.insert(Name::new(formatted_name));
            // --- Get Mesh ---
            let Some(data_link) = trishape_data.data_link else {
                /* Warn & return */
                return;
            };
            let Some(mesh_handle) = mesh_handles.get(&data_link).cloned() else {
                /* Warn & return */
                return;
            };

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
                        _ => {}
                    }
                }

                // `final_material` now holds the configured StandardMaterial struct.
                // If you need a handle later, you add it to the Assets collection *after* this:
                // let final_material_handle = materials_asset_server.add(final_material); // Assuming you have mutable access to Assets<StandardMaterial>
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

                    // *** Apply determined AlphaMode AFTER texture assignment ***
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
                    textured_material // This is a StandardMaterial struct
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
                    material_data // This is a StandardMaterial struct
                }
            };
            final_material.alpha_mode = AlphaMode::Mask(0.5);
            final_material.cull_mode = Some(Face::Back);
            let extension = ExtendedMaterial {
                base: final_material,
                extension: ColorKeyExtension {},
            };
            let final_material_handle = materials_colorkey.add(extension);
            let mut forward_mat: StandardMaterial = Color::srgb(0.1, 0.2, 0.1).into();

            // --- Insert Mesh and Material components ---
            let forward_mat_h = materials.add(forward_mat);
            entity_commands.insert((
                Mesh3d(mesh_handle), // Handle<Mesh>
                MeshMaterial3d(final_material_handle),
            ));
            if texture_was_bmp {
                info!(
                    "  TriShape {}: Adding ColorKeyExtension for BMP texture.",
                    nif_index
                );
                // If a BMP texture was applied, also insert the extension marker component.
                // Bevy's MaterialPlugin<StandardMaterial, ColorKeyExtension> will detect this
                // and use your custom color_key_extension.wgsl shader logic.
                // entity_commands.insert(ColorKeyExtension::default());
            }
        }
        _ => {
            // This block type isn't visually spawned in the hierarchy
            info!(
                "Skipping hierarchy spawn for block type at index {}",
                nif_index
            );
            entity_commands.despawn(); // Despawn the empty entity created
            entity_map.remove(&nif_index); // Remove from map
            should_keep_entity = false; // Don't try to parent it
        }
    }
    if should_keep_entity {
        commands.entity(parent_entity).add_child(current_entity_id); // <--- PARENTING HERE
        info!(
            "Parented block {} ({:?}) to parent entity {:?}",
            nif_index, current_entity_id, parent_entity
        );
    }
}
