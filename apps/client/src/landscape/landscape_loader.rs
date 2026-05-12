use crate::defs::{
    AllLandscapes, EsmReaderResource, LandTextureDefinitions, LandscapeAwaitingImages,
    LandscapeCell, LandscapeLoader,
};
use avian3d::prelude::{Collider, RigidBody};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::{
    asset::{AssetServer, Assets, Handle, RenderAssetUsages},
    image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    log::warn,
    math::Vec3,
    pbr::MeshMaterial3d,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    transform::components::Transform,
};
use esm_parser_lib::{
    EsmReader, LAND_NUM_TEXTURES, LAND_NUM_VERTS, LAND_SIDE_VERTICES,
    LAND_TEXTURE_SQUARES_PER_SIDE, LandTexture, LandVertexColor, LandVertexNormal,
};
use std::collections::{HashMap, HashSet};

use crate::const_defs::MORROWIND_CELL_SIZE;

use super::terrain_material::{TerrainBlendMaterial, TerrainMaterialParams};

pub fn load_landscape(
    landscape_loader_q: Query<(Entity, &LandscapeLoader), Added<LandscapeLoader>>,
    land_texture_definitions: Res<LandTextureDefinitions>,
    asset_server: Res<AssetServer>,
    mut reader_res: ResMut<EsmReaderResource>,
    mut landscapes: ResMut<AllLandscapes>,
    mut commands: Commands,
) {
    for (entity, landscape_loader) in landscape_loader_q.iter() {
        let reader = &mut reader_res.0;
        let current_cell_x = landscape_loader.cell_x;
        let current_cell_y = landscape_loader.cell_y;

        let Some(landscape) = landscapes.0.get_mut(&(current_cell_x, current_cell_y)) else {
            cancel_entity_landscape_load(
                &mut commands,
                entity,
                &format!(
                    "Tried to load landscape ({},{}) that doesn't exist in landscapes map!",
                    current_cell_x, current_cell_y
                ),
            );
            continue; // Continue to next entity in query
        };

        // Ensure detailed data is loaded for the current cell
        if landscape.detailed_data.is_none() {
            if let Err(e) = landscape.load_detailed_data(reader) {
                cancel_entity_landscape_load(
                    &mut commands,
                    entity,
                    &format!(
                        "Failed to load detailed landscape data for ({},{}): {}. Skipping.",
                        current_cell_x, current_cell_y, e
                    ),
                );
                continue;
            }
        }

        // Get all the unique texture ids we need to draw this cell
        let unique_ltex_ids_for_cell_and_border =
            get_required_texture_ids_for_cell_blendmap(&landscapes, current_cell_x, current_cell_y)
                .unwrap();

        let mut textures_for_material_setup: Vec<&LandTexture> = Vec::new();
        for ltex_id in unique_ltex_ids_for_cell_and_border.iter() {
            if let Some(land_texture_def) = land_texture_definitions.0.get(&(*ltex_id as u32)) {
                textures_for_material_setup.push(land_texture_def);
            } else {
                warn!(
                    "Missing LandTexture definition for LTEX ID: {} in cell ({},{}) during material setup.",
                    ltex_id, current_cell_x, current_cell_y
                );
            }
        }

        // start loading all the images for these texture ids
        let mut image_handles = Vec::new();
        let mut texture_index_map = HashMap::new();
        for (slot_index, land_texture_def) in textures_for_material_setup.iter().enumerate() {
            let texture_path = format!("Textures/{}", land_texture_def.texture_file);
            println!("trying to load {}", texture_path);
            image_handles.push(asset_server.load(texture_path));
            texture_index_map.insert(land_texture_def.index as u16, slot_index);
        }

        if image_handles.is_empty() && !unique_ltex_ids_for_cell_and_border.is_empty() {
            warn!(
                "Cell ({},{}) ended up with no image handles despite having unique texture IDs. Check LandTexture definitions and asset paths.",
                current_cell_x, current_cell_y
            );
        }
        commands.entity(entity).insert(LandscapeAwaitingImages {
            image_handles,
            texture_index_map,
        });
    }
}

pub fn setup_landscape_images(
    landscape_awaiting_images_q: Query<(Entity, &LandscapeLoader, &LandscapeAwaitingImages)>,
    mut images: ResMut<Assets<Image>>,
    mut all_landscapes: ResMut<AllLandscapes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut custom_materials: ResMut<Assets<TerrainBlendMaterial>>,
    mut commands: Commands,
    mut reader_res: ResMut<EsmReaderResource>,
) {
    // check to make sure texture images are loaded
    for (entity, landscape_loader, landscape_awaiting_image) in landscape_awaiting_images_q {
        for image_handle in &landscape_awaiting_image.image_handles {
            if images.get(image_handle).is_none() {
                return;
            }
        }

        // if we get here, we have all the images loaded
        // create 1 alpha map per texture that corresponds to areas the texture is "enabled"
        let alpha_map_array = match generate_alpha_maps_for_active_layers_new(
            &all_landscapes,
            landscape_loader.cell_x,
            landscape_loader.cell_y,
            &landscape_awaiting_image.image_handles,
            &landscape_awaiting_image.texture_index_map,
            2,
        ) {
            Ok(arr) => arr,
            Err(err) => {
                cancel_entity_landscape_load(&mut commands, entity, &err);
                return;
            }
        };
        let array_len = alpha_map_array.len();
        let mut alpha_2d_image_array = setup_2d_image_array(
            landscape_awaiting_image.image_handles.len() as u32,
            alpha_map_array,
        );
        alpha_2d_image_array.texture_descriptor.usage =
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
        alpha_2d_image_array.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            ..Default::default()
        });
        let alpha_texture_array_h = images.add(alpha_2d_image_array);

        //find the loaded images and assign them to a 2d texture array
        let texture_array_image = setup_terrain_2d_array(landscape_awaiting_image, &images);
        let texture_handle: Handle<Image> = images.add(texture_array_image);
        commands.entity(entity).remove::<LandscapeAwaitingImages>();
        finish_landscape_setup(
            entity,
            &mut commands,
            &mut meshes,
            &mut custom_materials,
            alpha_texture_array_h,
            texture_handle,
            array_len,
            (landscape_loader.cell_x, landscape_loader.cell_y),
            &mut all_landscapes,
            &mut reader_res.0,
        );
    }
}

fn finish_landscape_setup(
    landscape_entity: Entity,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    custom_materials: &mut ResMut<Assets<TerrainBlendMaterial>>,
    alpha_texture_array_h: Handle<Image>,
    texture_handle: Handle<Image>,
    image_handles_array_len: usize,
    cell_position: (i32, i32),
    all_landscapes: &mut AllLandscapes,
    reader: &mut EsmReader,
) {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    let Some(landscape_data) =
        construct_landscape_mesh(cell_position.0, cell_position.1, all_landscapes, reader)
    else {
        return;
    };
    let (positions_bevy, normals_bevy, uvs_bevy, colors_bevy, indices) = landscape_data;
    let heightfield_collider = create_heightfield_collider(&positions_bevy);

    //Old, we use a heightfield now
    /*
     * let positions_vec3_for_collider: Vec<Vec3> = positions_bevy
        .iter()
        .map(|pos_array| Vec3::from_array(*pos_array))
        .collect();
    let indices_vec3_for_collider: Vec<[u32; 3]> = indices
        .chunks_exact(3) // Creates an iterator over slices of 3 elements (&[u32])
        .map(|chunk| {
            // Convert the slice chunk [idx0, idx1, idx2] into an array [u32; 3]
            // This assumes chunk will always have exactly 3 elements due to chunks_exact
            [chunk[0], chunk[1], chunk[2]]
        })
        .collect();
    */
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions_bevy);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals_bevy);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs_bevy);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors_bevy);
    mesh.insert_indices(Indices::U32(indices));
    if let Err(e) = mesh.generate_tangents() {
        warn!("error generating tangents while creating landscape: {}", e);
    }
    let mesh_h = meshes.add(mesh);
    let base_vtex_grid_dim = LAND_TEXTURE_SQUARES_PER_SIDE as f32; // 16

    let material_handle = custom_materials.add(TerrainBlendMaterial {
        source_textures: texture_handle,
        alpha_map_array: alpha_texture_array_h,
        params: TerrainMaterialParams {
            num_textures_in_array: image_handles_array_len as u32, // The number of unique textures
            diffuse_tiling_factor: 16.0,
            base_vtex_grid_dim,
        },
    });

    commands.entity(landscape_entity).insert((
        Visibility::Visible,
        Name::new("land"),
        LandscapeCell {
            x: cell_position.0,
            y: cell_position.1,
        },
        Mesh3d(mesh_h),
        MeshMaterial3d(material_handle),
        //Collider::trimesh(positions_vec3_for_collider, indices_vec3_for_collider).unwrap(),
        RigidBody::Fixed,
        heightfield_collider,
    ));
    let cell_translation = Vec3::new(
        -cell_position.0 as f32 * MORROWIND_CELL_SIZE,
        0.0,
        cell_position.1 as f32 * MORROWIND_CELL_SIZE,
    );

    commands
        .entity(landscape_entity)
        .remove::<LandscapeLoader>();
    commands
        .entity(landscape_entity)
        .insert(Transform::from_translation(cell_translation));
}
pub fn construct_landscape_mesh(
    cell_grid_x: i32,
    cell_grid_y: i32,
    all_landscapes: &mut AllLandscapes, // Mutable to allow loading/mutating neighbor data
    reader: &mut EsmReader,
) -> Option<(
    Vec<[f32; 3]>, // positions_bevy
    Vec<[f32; 3]>, // normals_bevy
    Vec<[f32; 2]>, // uvs_bevy
    Vec<[f32; 4]>, // final_vclr_bevy
    Vec<u32>,      // indices
)> {
    // --- Step 0: Ensure current cell's detailed data is loaded if necessary ---
    // This is a short mutable operation before the main read phase.
    let current_cell_key = (cell_grid_x, cell_grid_y);
    let landscape_entry_for_current_cell = all_landscapes.0.get_mut(&current_cell_key)?; // Get mut to potentially load
    if landscape_entry_for_current_cell.detailed_data.is_none() {
        if landscape_entry_for_current_cell
            .load_detailed_data(reader)
            .is_err()
        {
            // warn!("Failed to load detailed_data for primary cell ({}, {})", cell_grid_x, cell_grid_y);
            return None; // Or handle error appropriately
        }
    }
    // --- Pass 1: Read-only data extraction from the (now hopefully populated) current cell ---
    let (
        morrowind_vertex_z_values, // Owned Vec<f32>
        initial_esm_normals,       // Owned Vec<LandVertexNormal>
        initial_esm_colors,        // Owned Vec<LandVertexColor>
    ) = {
        let landscape_entry = all_landscapes.0.get(&current_cell_key)?;
        let landscape_detailed_data = landscape_entry.detailed_data.as_ref()?;
        let vhgt_data = landscape_detailed_data.vertex_heights.as_ref()?;

        let z_values = vhgt_data.heights.clone();

        // 1b. Extract initial per-vertex normals and colors into new owned Vecs
        let mut temp_normals = Vec::with_capacity(LAND_NUM_VERTS);
        let mut temp_colors = Vec::with_capacity(LAND_NUM_VERTS);

        let normals_data_source = landscape_detailed_data.vertex_normals.as_ref();
        let colors_data_source = landscape_detailed_data.vertex_colors.as_ref();

        for i in 0..LAND_NUM_VERTS {
            temp_normals.push(normals_data_source.map_or_else(
                || LandVertexNormal { x: 0, y: 0, z: 127 },
                |data| {
                    data.get(i)
                        .copied()
                        .unwrap_or(LandVertexNormal { x: 0, y: 0, z: 127 })
                },
            ));
            temp_colors.push(colors_data_source.map_or_else(
                || LandVertexColor {
                    r: 128,
                    g: 128,
                    b: 128,
                },
                |data| {
                    data.get(i).copied().unwrap_or(LandVertexColor {
                        r: 128,
                        g: 128,
                        b: 128,
                    })
                },
            ));
        }
        (z_values, temp_normals, temp_colors)
    }; // Immutable borrow on `all_landscapes.0` (from `landscape_entry`) ends here.

    // --- Pass 2: Calculate final vertex attributes, performing mutations on `all_landscapes` as needed ---
    let mut positions_bevy: Vec<[f32; 3]> = Vec::with_capacity(LAND_NUM_VERTS);
    let mut uvs_bevy: Vec<[f32; 2]> = Vec::with_capacity(LAND_NUM_VERTS);
    let mut final_normals_bevy: Vec<[f32; 3]> = vec![[0.0, 1.0, 0.0]; LAND_NUM_VERTS];
    let mut final_vclr_bevy: Vec<[f32; 4]> = vec![[1.0, 1.0, 1.0, 1.0]; LAND_NUM_VERTS];

    let lx_max_idx = LAND_SIDE_VERTICES - 1;

    for y_idx_mesh in 0..LAND_SIDE_VERTICES {
        for x_idx_mesh in 0..LAND_SIDE_VERTICES {
            let vertex_index_flat = y_idx_mesh * LAND_SIDE_VERTICES + x_idx_mesh;

            // Positions and UVs (from Pass 1 data or calculated here)
            let half_cell = MORROWIND_CELL_SIZE / 2.0;

            let mw_x_pos_relative =
                (x_idx_mesh as f32 / lx_max_idx as f32 * MORROWIND_CELL_SIZE) - half_cell;
            let mw_y_pos_relative =
                (y_idx_mesh as f32 / lx_max_idx as f32 * MORROWIND_CELL_SIZE) - half_cell;
            let mw_z_pos = morrowind_vertex_z_values[vertex_index_flat];
            positions_bevy.push([-mw_x_pos_relative, mw_z_pos, mw_y_pos_relative]);

            let norm_u_mesh = x_idx_mesh as f32 / lx_max_idx as f32;
            let norm_v_mesh = y_idx_mesh as f32 / lx_max_idx as f32;
            uvs_bevy.push([norm_u_mesh, 1.0 - norm_v_mesh]);

            // Use initial normals and colors from Pass 1
            let mut esm_normal_to_use = initial_esm_normals[vertex_index_flat];
            let mut esm_color_to_use = initial_esm_colors[vertex_index_flat];

            let is_corner = (x_idx_mesh == 0 || x_idx_mesh == lx_max_idx)
                && (y_idx_mesh == 0 || y_idx_mesh == lx_max_idx);

            if is_corner {
                let mut sum_mw_normals_vec3 = Vec3::ZERO;
                let mut contributing_normals_count = 0;
                let conceptual_neighbors = [
                    (x_idx_mesh as i32, y_idx_mesh as i32 - 1),
                    (x_idx_mesh as i32, y_idx_mesh as i32 + 1),
                    (x_idx_mesh as i32 - 1, y_idx_mesh as i32),
                    (x_idx_mesh as i32 + 1, y_idx_mesh as i32),
                ];
                for (eff_x, eff_y) in conceptual_neighbors.iter() {
                    let (actual_cell_x, actual_cell_y, resolved_local_vx, resolved_local_vy) =
                        resolve_conceptual_vertex_coords(cell_grid_x, cell_grid_y, *eff_x, *eff_y);

                    // Mutable calls are now safe
                    if let Some(n_esm) = get_raw_normal_from_cell(
                        all_landscapes,
                        reader,
                        actual_cell_x,
                        actual_cell_y,
                        resolved_local_vx,
                        resolved_local_vy,
                    ) {
                        sum_mw_normals_vec3 += Vec3::new(
                            n_esm.x as f32 / 127.0,
                            n_esm.y as f32 / 127.0,
                            n_esm.z as f32 / 127.0,
                        );
                        contributing_normals_count += 1;
                    }
                }
                if contributing_normals_count > 0 {
                    let avg_mw_normal = (sum_mw_normals_vec3 / contributing_normals_count as f32)
                        .normalize_or_zero();
                    final_normals_bevy[vertex_index_flat] =
                        transform_mw_normal_to_bevy(avg_mw_normal).to_array();
                } else {
                    final_normals_bevy[vertex_index_flat] = transform_mw_normal_to_bevy(
                        Vec3::new(
                            esm_normal_to_use.x as f32 / 127.0,
                            esm_normal_to_use.y as f32 / 127.0,
                            esm_normal_to_use.z as f32 / 127.0,
                        )
                        .normalize_or_zero(),
                    )
                    .to_array();
                }
            } else {
                // Non-corner
                let (actual_cell_x, actual_cell_y, resolved_local_vx, resolved_local_vy) =
                    resolve_conceptual_vertex_coords(
                        cell_grid_x,
                        cell_grid_y,
                        x_idx_mesh as i32,
                        y_idx_mesh as i32,
                    );
                if let Some(fixed_esm_normal) = get_raw_normal_from_cell(
                    // Mutable call
                    all_landscapes,
                    reader,
                    actual_cell_x,
                    actual_cell_y,
                    resolved_local_vx,
                    resolved_local_vy,
                ) {
                    esm_normal_to_use = fixed_esm_normal; // Update the base normal for final output
                }
                final_normals_bevy[vertex_index_flat] = transform_mw_normal_to_bevy(
                    Vec3::new(
                        esm_normal_to_use.x as f32 / 127.0,
                        esm_normal_to_use.y as f32 / 127.0,
                        esm_normal_to_use.z as f32 / 127.0,
                    )
                    .normalize_or_zero(),
                )
                .to_array();
            }

            if x_idx_mesh == lx_max_idx || y_idx_mesh == lx_max_idx {
                // Color fixing
                let (actual_cell_x, actual_cell_y, resolved_local_vx, resolved_local_vy) =
                    resolve_conceptual_vertex_coords(
                        cell_grid_x,
                        cell_grid_y,
                        x_idx_mesh as i32,
                        y_idx_mesh as i32,
                    );
                if let Some(fixed_esm_color) = get_raw_color_from_cell(
                    // Mutable call
                    all_landscapes,
                    reader,
                    actual_cell_x,
                    actual_cell_y,
                    resolved_local_vx,
                    resolved_local_vy,
                ) {
                    esm_color_to_use = fixed_esm_color;
                }
            }
            final_vclr_bevy[vertex_index_flat] = [
                esm_color_to_use.r as f32 / 255.0,
                esm_color_to_use.g as f32 / 255.0,
                esm_color_to_use.b as f32 / 255.0,
                1.0,
            ];
        }
    }

    // --- Index Buffer Generation ---
    let mut indices_list = Vec::new();
    let quads_per_side = LAND_SIDE_VERTICES - 1;
    let n_verts_per_side_u32 = LAND_SIDE_VERTICES as u32;
    for qy_idx in 0..quads_per_side {
        for qx_idx in 0..quads_per_side {
            let v_a = qy_idx as u32 * n_verts_per_side_u32 + qx_idx as u32;
            let v_b = qy_idx as u32 * n_verts_per_side_u32 + (qx_idx + 1) as u32;
            let v_c = (qy_idx + 1) as u32 * n_verts_per_side_u32 + qx_idx as u32;
            let v_d = (qy_idx + 1) as u32 * n_verts_per_side_u32 + (qx_idx + 1) as u32;
            if (qy_idx + (qx_idx % 2)) % 2 == 1 {
                indices_list.extend_from_slice(&[v_b, v_d, v_c, v_a, v_b, v_c]);
            } else {
                indices_list.extend_from_slice(&[v_a, v_d, v_c, v_a, v_b, v_d]);
            }
        }
    }
    Some((
        positions_bevy,
        final_normals_bevy,
        uvs_bevy,
        final_vclr_bevy,
        indices_list,
    ))
}
fn setup_terrain_2d_array(
    landscape_awaiting_image: &LandscapeAwaitingImages,
    all_images: &ResMut<Assets<Image>>,
) -> Image {
    let mut texture_images = Vec::new();
    for handle in landscape_awaiting_image.image_handles.iter() {
        if let Some(image) = all_images.get(handle) {
            texture_images.push(image.clone());
        }
    }
    let mut texture_array_image = setup_2d_image_array(
        landscape_awaiting_image.image_handles.len() as u32,
        texture_images,
    );

    texture_array_image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 4,
        ..Default::default()
    });
    texture_array_image
}
fn create_heightfield_collider(positions: &[[f32; 3]]) -> Collider {
    let mut grid = Vec::with_capacity(LAND_SIDE_VERTICES);
    for x in (0..LAND_SIDE_VERTICES).rev() {
        let mut row: Vec = Vec::with_capacity(LAND_SIDE_VERTICES);
        for y in 0..LAND_SIDE_VERTICES {
            let vertex_index = y * LAND_SIDE_VERTICES + x;
            let height = if vertex_index < positions.len() {
                positions[vertex_index][1] // Use Y as height
            } else {
                0.0
            };
            row.push(height);
        }
        grid.push(row);
    }
    let scale_xz = MORROWIND_CELL_SIZE;
    let scale = Vec3::new(scale_xz, 1.0, scale_xz);

    Collider::heightfield(grid, scale)
}
fn setup_2d_image_array(mut len: u32, image_array: Vec<Image>) -> Image {
    //concat all images into 1 u8 vec
    println!("len: {}", len);
    let mut all_alpha_array_pixel_data = Vec::new();
    let mut image_width = 0;
    let mut image_height = 0;
    let mut image_format = TextureFormat::R8Unorm; //just a default, this is modified in the loop

    for image in image_array {
        // TODO:: find some way to not use this fix for wgsl shader not making this a 2d array
        // unless it has at least 2 textures
        // for now we just duplicate the image so it's always a 2d array
        if len < 2 {
            all_alpha_array_pixel_data.extend_from_slice(&image.data.clone().unwrap());
            len = 2;
        }
        all_alpha_array_pixel_data.extend_from_slice(&image.data.clone().unwrap());
        image_width = image.width();
        image_height = image.height();
        image_format = image.texture_descriptor.format;
    }
    let alpha_2d_image_array = Image::new(
        Extent3d {
            width: image_width,
            height: image_height,
            depth_or_array_layers: len,
        },
        TextureDimension::D2,
        all_alpha_array_pixel_data,
        image_format,
        RenderAssetUsages::RENDER_WORLD,
    );
    alpha_2d_image_array
}

fn resolve_conceptual_vertex_coords(
    base_cell_x: i32,
    base_cell_y: i32,
    conceptual_local_vx: i32,
    conceptual_local_vy: i32,
) -> (
    i32,   /*actual_cell_x*/
    i32,   /*actual_cell_y*/
    usize, /*resolved_local_vx*/
    usize, /*resolved_local_vy*/
) {
    let mut actual_cell_x = base_cell_x;
    let mut actual_cell_y = base_cell_y;
    let mut resolved_local_vx = conceptual_local_vx;
    let mut resolved_local_vy = conceptual_local_vy;

    let side_vertices_i32 = LAND_SIDE_VERTICES as i32;

    // Resolve X coordinate
    while resolved_local_vx >= side_vertices_i32 {
        actual_cell_x += 1;
        resolved_local_vx -= side_vertices_i32;
    }
    while resolved_local_vx < 0 {
        actual_cell_x -= 1;
        resolved_local_vx += side_vertices_i32;
    }

    // Resolve Y coordinate
    while resolved_local_vy >= side_vertices_i32 {
        actual_cell_y += 1;
        resolved_local_vy -= side_vertices_i32;
    }
    while resolved_local_vy < 0 {
        actual_cell_y -= 1;
        resolved_local_vy += side_vertices_i32;
    }

    (
        actual_cell_x,
        actual_cell_y,
        resolved_local_vx as usize,
        resolved_local_vy as usize,
    )
}
fn cancel_entity_landscape_load(commands: &mut Commands, entity: Entity, err: &str) {
    commands.entity(entity).remove::<LandscapeAwaitingImages>();
    commands.entity(entity).remove::<LandscapeLoader>();
    warn!("landscape loading failed! {}", err);
}
fn transform_mw_normal_to_bevy(mw_normal: Vec3) -> Vec3 {
    Vec3::new(-mw_normal.x, mw_normal.z, mw_normal.y).normalize_or_zero()
}

fn get_raw_normal_from_cell(
    all_landscapes: &mut AllLandscapes,
    reader: &mut EsmReader,
    cell_x: i32,
    cell_y: i32,
    local_vx: usize, // Local vertex X-index within the cell (0-64)
    local_vy: usize, // Local vertex Y-index within the cell (0-64)
) -> Option<LandVertexNormal> {
    if let Some(landscape) = all_landscapes.0.get_mut(&(cell_x, cell_y)) {
        if landscape.detailed_data.is_none() {
            // In a real Bevy system, this mutable borrow might be tricky.
            // Loading might happen in a prior system, or Landscape uses interior mutability.
            if landscape.load_detailed_data(reader).is_err() {
                return None;
            }
        }
        return landscape
            .detailed_data
            .as_ref()
            .and_then(|payload| payload.vertex_normals.as_ref())
            .map(|normals_data| {
                let flat_idx = local_vy * LAND_SIDE_VERTICES + local_vx;
                normals_data.get(flat_idx).copied() // Use .get() for safety
            })
            .flatten();
    }
    None
}
fn get_raw_color_from_cell(
    all_landscapes: &mut AllLandscapes,
    reader: &mut EsmReader,
    cell_x: i32,
    cell_y: i32,
    local_vx: usize,
    local_vy: usize,
) -> Option<LandVertexColor> {
    if let Some(landscape) = all_landscapes.0.get_mut(&(cell_x, cell_y)) {
        if landscape.detailed_data.is_none() {
            if landscape.load_detailed_data(reader).is_err() {
                return None;
            }
        }
        return landscape
            .detailed_data
            .as_ref()
            .and_then(|payload| payload.vertex_colors.as_ref())
            .map(|colors_data| {
                let flat_idx = local_vy * LAND_SIDE_VERTICES + local_vx;
                colors_data.get(flat_idx).copied()
            })
            .flatten();
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CellSample {
    cell_x: i32,
    cell_y: i32,
    src_row_in_cell: usize,
    src_col_in_cell: usize,
    dst_row_on_grid: usize,
    dst_col_on_grid: usize,
}
fn get_blendmap_local_range(
    current_cell_axis_coord: i32,
    min_overall_cell_axis_coord_for_sampling_block: i32,
    texture_quad_dimension: usize,
    min_overall_tex_axis_coord_for_chunk: i32,
    max_overall_tex_axis_coord_for_chunk: i32,
) -> (usize, usize) {
    let cell_min_tex_coord_abs = (current_cell_axis_coord
        - min_overall_cell_axis_coord_for_sampling_block)
        * texture_quad_dimension as i32;
    let cell_max_tex_coord_abs = cell_min_tex_coord_abs + texture_quad_dimension as i32 - 1;

    let local_begin_tex_coord = if min_overall_tex_axis_coord_for_chunk > cell_min_tex_coord_abs {
        (min_overall_tex_axis_coord_for_chunk - cell_min_tex_coord_abs) as usize
    } else {
        0
    };

    let local_end_tex_coord_inclusive =
        if cell_max_tex_coord_abs < max_overall_tex_axis_coord_for_chunk {
            texture_quad_dimension - 1
        } else {
            (max_overall_tex_axis_coord_for_chunk - cell_min_tex_coord_abs).max(0) as usize
        };

    let mut local_end_tex_coord_exclusive = local_end_tex_coord_inclusive + 1;

    if local_begin_tex_coord >= local_end_tex_coord_exclusive {
        local_end_tex_coord_exclusive = local_begin_tex_coord;
    }

    (
        local_begin_tex_coord.min(texture_quad_dimension),
        local_end_tex_coord_exclusive.min(texture_quad_dimension),
    )
}

pub fn generate_alpha_maps_for_active_layers_new(
    all_landscapes: &AllLandscapes,
    current_cell_grid_x: i32,
    current_cell_grid_y: i32,
    image_handles_for_cell: &Vec<Handle<Image>>, // Using Bevy Handle and Image
    ltex_id_to_slot_index_map: &HashMap<u16, usize>,
    image_scale_factor: u32,
) -> Result<Vec<Image>, String> {
    // Returning Vec<Image>
    if image_scale_factor == 0 {
        return Err("image_scale_factor cannot be zero.".to_string());
    }

    let num_layers = image_handles_for_cell.len();
    if num_layers == 0 {
        return Ok(Vec::new());
    }

    let blendmap_grid_dim = LAND_TEXTURE_SQUARES_PER_SIDE + 1;
    let final_alpha_map_dim = blendmap_grid_dim as u32 * image_scale_factor;

    // Create output images using Bevy's Image::new
    let mut output_alpha_maps: Vec<Image> = (0..num_layers)
        .map(|_| {
            let initial_pixel_data =
                vec![0u8; (final_alpha_map_dim * final_alpha_map_dim) as usize];
            let mut alpha_map_image = Image::new(
                Extent3d {
                    width: final_alpha_map_dim,
                    height: final_alpha_map_dim,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                initial_pixel_data,
                TextureFormat::R8Unorm, // Correct format for single channel alpha
                RenderAssetUsages::RENDER_WORLD, // Added from your snippet
            );
            // Set texture usages as per your snippet
            alpha_map_image.texture_descriptor.usage =
                TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
            alpha_map_image
        })
        .collect();

    let mut sampled_grid_texture_ids =
        vec![DEFAULT_TEXTURE_ID; blendmap_grid_dim * blendmap_grid_dim];

    let sample_handler = |sample: CellSample| {
        let texture_id = get_texture_id_from_landscape(
            all_landscapes,
            sample.cell_x,
            sample.cell_y,
            sample.src_row_in_cell,
            sample.src_col_in_cell,
        );
        if sample.dst_col_on_grid < blendmap_grid_dim && sample.dst_row_on_grid < blendmap_grid_dim
        {
            sampled_grid_texture_ids
                [sample.dst_col_on_grid * blendmap_grid_dim + sample.dst_row_on_grid] = texture_id;
        }
    };

    sample_textures_for_single_cell_grid(
        current_cell_grid_x,
        current_cell_grid_y,
        LAND_TEXTURE_SQUARES_PER_SIDE,
        sample_handler,
    )?;
    dbg!(&sampled_grid_texture_ids[0]); // What is the ID for the P1 vertex?
    dbg!(ltex_id_to_slot_index_map.get(&sampled_grid_texture_ids[0])); // Which slot does it map to?

    // let mut active_slots_painted: HashSet<usize> = HashSet::new(); // Can be used by caller if needed

    for y_grid_idx in 0..blendmap_grid_dim {
        for x_grid_idx in 0..blendmap_grid_dim {
            let current_ltex_id =
                sampled_grid_texture_ids[y_grid_idx * blendmap_grid_dim + x_grid_idx];

            if let Some(&slot_index) = ltex_id_to_slot_index_map.get(&current_ltex_id) {
                if slot_index < num_layers {
                    // active_slots_painted.insert(slot_index); // Track if needed
                    let target_image = &mut output_alpha_maps[slot_index];

                    let real_y_base =
                        (blendmap_grid_dim - 1 - y_grid_idx) * image_scale_factor as usize;
                    let real_x_base = x_grid_idx * image_scale_factor as usize;

                    for y_offset in 0..image_scale_factor as usize {
                        for x_offset in 0..image_scale_factor as usize {
                            let pixel_y = real_y_base + y_offset;
                            let pixel_x = real_x_base + x_offset;
                            if pixel_y < final_alpha_map_dim as usize
                                && pixel_x < final_alpha_map_dim as usize
                            {
                                // Image.data is Vec<u8>
                                target_image.data.as_mut().unwrap()
                                    [pixel_y * final_alpha_map_dim as usize + pixel_x] = 255;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(output_alpha_maps)
}
fn sample_textures_for_single_cell_grid<F>(
    target_cell_x: i32,
    target_cell_y: i32,
    texture_quad_dimension: usize,
    mut f_sample_handler: F,
) -> Result<(), String>
where
    F: FnMut(CellSample),
{
    let min_world_x_for_sampling = target_cell_x as f32;
    let min_world_y_for_sampling = target_cell_y as f32;

    let mut min_row_tex_coord = ((min_world_x_for_sampling - min_world_x_for_sampling.floor())
        * (texture_quad_dimension as f32 + 1.0)) as i32
        - 1;
    let min_col_tex_coord = ((min_world_y_for_sampling - min_world_y_for_sampling.floor())
        * (texture_quad_dimension as f32 + 1.0)) as i32;

    let mut sampling_origin_cell_x = min_world_x_for_sampling.floor() as i32;
    if min_row_tex_coord < 0 {
        sampling_origin_cell_x -= 1;
        min_row_tex_coord += texture_quad_dimension as i32;
    }
    let sampling_origin_cell_y = min_world_y_for_sampling.floor() as i32;

    // --- MODIFICATION START ---
    // These define the inclusive end of the *texture coordinate indices* we are interested in
    // to generate (texture_quad_dimension + 1) sample points.
    // Corresponds to `minCoord + num_quads` from C++'s `maxRow = minRow + textureSize * size` logic.
    let max_row_tex_coord_for_range_input = min_row_tex_coord + texture_quad_dimension as i32;
    let max_col_tex_coord_for_range_input = min_col_tex_coord + texture_quad_dimension as i32;
    // --- MODIFICATION END ---

    let denominator = texture_quad_dimension as i32;

    // Numerator for end_cell_offset should be based on the number of points, not max index.
    // The number of points in the range is max_coord_for_range_input - min_coord + 1
    let num_points_x = max_row_tex_coord_for_range_input - min_row_tex_coord + 1; // This should be 17
    let end_cell_offset_x = if denominator == 0 {
        0
    } else {
        (num_points_x + denominator - 1) / denominator
    };

    let num_points_y = max_col_tex_coord_for_range_input - min_col_tex_coord + 1; // This should be 17 or 16
    let end_cell_offset_y = if denominator == 0 {
        0
    } else {
        (num_points_y + denominator - 1) / denominator
    };

    let overall_max_cell_x_for_sampling = sampling_origin_cell_x + end_cell_offset_x - 1;
    let overall_max_cell_y_for_sampling = sampling_origin_cell_y + end_cell_offset_y - 1;

    let mut base_dst_col_on_grid: usize = 0;
    for current_sampling_cell_y in sampling_origin_cell_y..=overall_max_cell_y_for_sampling {
        let mut base_dst_row_on_grid: usize = 0;

        let (local_begin_col_tex, local_end_col_tex_exclusive) = get_blendmap_local_range(
            current_sampling_cell_y,
            sampling_origin_cell_y,
            texture_quad_dimension,
            min_col_tex_coord,
            max_col_tex_coord_for_range_input, // Use updated max
        );
        let col_count_in_strip = local_end_col_tex_exclusive.saturating_sub(local_begin_col_tex);
        if col_count_in_strip == 0
            && !(local_begin_col_tex == 0 && local_end_col_tex_exclusive == 0)
        {
            // This continue might be too aggressive if an empty range is valid but not at (0,0)
            // C++ simply lets the loop not iterate if count is 0.
            // For safety, let's ensure the loop just doesn't run if count is 0.
            // The saturating_sub handles this.
        }

        for current_sampling_cell_x in sampling_origin_cell_x..=overall_max_cell_x_for_sampling {
            let (local_begin_row_tex, local_end_row_tex_exclusive) = get_blendmap_local_range(
                current_sampling_cell_x,
                sampling_origin_cell_x,
                texture_quad_dimension,
                min_row_tex_coord,
                max_row_tex_coord_for_range_input, // Use updated max
            );
            let row_count_in_block =
                local_end_row_tex_exclusive.saturating_sub(local_begin_row_tex);
            if row_count_in_block == 0
                && !(local_begin_row_tex == 0 && local_end_row_tex_exclusive == 0)
            {
                // See comment above for col_count_in_strip
            }

            for col_idx_in_strip_block in 0..col_count_in_strip {
                for row_idx_in_cell_block in 0..row_count_in_block {
                    f_sample_handler(CellSample {
                        cell_x: current_sampling_cell_x,
                        cell_y: current_sampling_cell_y,
                        src_row_in_cell: local_begin_row_tex + row_idx_in_cell_block,
                        src_col_in_cell: local_begin_col_tex + col_idx_in_strip_block,
                        dst_row_on_grid: base_dst_row_on_grid + row_idx_in_cell_block,
                        dst_col_on_grid: base_dst_col_on_grid + col_idx_in_strip_block,
                    });
                }
            }
            base_dst_row_on_grid += row_count_in_block;
        }
        base_dst_col_on_grid += col_count_in_strip;
    }
    Ok(())
}
const DEFAULT_TEXTURE_ID: u16 = u16::MAX;
fn get_texture_id_from_landscape(
    all_landscapes: &AllLandscapes,
    cell_x: i32,
    cell_y: i32,
    src_row_in_cell: usize, // X-like coord in cell's texture grid
    src_col_in_cell: usize, // Y-like coord in cell's texture grid
) -> u16 {
    if src_row_in_cell >= LAND_TEXTURE_SQUARES_PER_SIDE
        || src_col_in_cell >= LAND_TEXTURE_SQUARES_PER_SIDE
    {
        return DEFAULT_TEXTURE_ID;
    }
    match all_landscapes.0.get(&(cell_x, cell_y)) {
        Some(landscape_cell_data) => {
            match landscape_cell_data.texture_indices.as_ref() {
                Some(texture_indices_array) => {
                    // Index: y * width + x
                    let index = src_col_in_cell * LAND_TEXTURE_SQUARES_PER_SIDE + src_row_in_cell;
                    if index < LAND_NUM_TEXTURES {
                        texture_indices_array[index]
                    } else {
                        DEFAULT_TEXTURE_ID
                    }
                }
                None => DEFAULT_TEXTURE_ID,
            }
        }
        None => DEFAULT_TEXTURE_ID,
    }
}

pub fn get_required_texture_ids_for_cell_blendmap(
    all_landscapes: &AllLandscapes,
    target_cell_x: i32,
    target_cell_y: i32,
) -> Result<HashSet<u16>, String> {
    let mut unique_ltex_ids = HashSet::new();

    // This closure will be called for every sample point.
    // It fetches the texture ID and adds it to our set.
    let sample_handler = |sample: CellSample| {
        let texture_id = get_texture_id_from_landscape(
            all_landscapes,
            sample.cell_x,
            sample.cell_y,
            sample.src_row_in_cell,
            sample.src_col_in_cell,
        );
        unique_ltex_ids.insert(texture_id);
    };

    // Perform the sampling simulation
    sample_textures_for_single_cell_grid(
        target_cell_x,
        target_cell_y,
        LAND_TEXTURE_SQUARES_PER_SIDE,
        sample_handler,
    )?;

    Ok(unique_ltex_ids)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_get_required_ids_single_texture_cell() {
        let tex_id_1 = 10u16;
        let mut landscape_map = HashMap::new();
        let mut landscape = Landscape::default();
        landscape.grid_x = 0;
        landscape.grid_y = 0;
        landscape.texture_indices = Some(Box::new(
            [tex_id_1; LAND_TEXTURE_SQUARES_PER_SIDE * LAND_TEXTURE_SQUARES_PER_SIDE],
        ));
        landscape_map.insert((0, 0), landscape);
        let all_landscapes =
            create_test_all_landscapes_for_id_scan(landscape_map.into_iter().collect());

        let result = get_required_texture_ids_for_cell_blendmap(&all_landscapes, 0, 0);
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 2); // 1 + default texture from neighbors
        assert!(ids.contains(&tex_id_1));
    }

    #[test]
    fn test_get_required_ids_mixed_textures_and_neighbor() {
        let tex_id_1 = 10u16;
        let tex_id_2 = 20u16;
        let tex_id_neighbor = 30u16; // This texture is in the cell to the left
        let tex_id_neighbor_not_included = 40u16; // This texture is in the cell to the left
        let landscape_1_coords = (0, 0);
        let landscape_2_coords = (-1, 0);
        let mut landscape_1_textures =
            [tex_id_1; LAND_TEXTURE_SQUARES_PER_SIDE * LAND_TEXTURE_SQUARES_PER_SIDE];
        landscape_1_textures[0] = tex_id_2; // Mix it up a bit

        let mut landscape_map = HashMap::new();
        let landscape_1 = Landscape {
            grid_x: landscape_1_coords.0,
            grid_y: landscape_1_coords.1,
            texture_indices: Some(Box::new(landscape_1_textures)),
            ..Default::default()
        };
        landscape_map.insert(landscape_1_coords, landscape_1);

        let mut landscape_2_textures =
            [tex_id_neighbor; LAND_TEXTURE_SQUARES_PER_SIDE * LAND_TEXTURE_SQUARES_PER_SIDE];
        landscape_2_textures[52] = tex_id_neighbor_not_included; // change a random coordinate in the middle

        let landscape_2 = Landscape {
            grid_x: landscape_2_coords.0,
            grid_y: landscape_2_coords.1,
            texture_indices: Some(Box::new(landscape_2_textures)),
            ..Default::default()
        };
        landscape_map.insert(landscape_2_coords, landscape_2);

        let all_landscapes =
            create_test_all_landscapes_for_id_scan(landscape_map.into_iter().collect());
        let result = get_required_texture_ids_for_cell_blendmap(&all_landscapes, 0, 0);
        assert!(result.is_ok());
        let ids = result.unwrap();

        // Expect to find tex_id_1, tex_id_2 from cell (0,0)
        // And tex_id_neighbor from cell (-1,0) because the sampling for vertices on the
        // left edge of cell (0,0) will look at the last column of textures in cell (-1,0).
        assert!(ids.contains(&tex_id_1), "Missing tex_id_1");
        assert!(ids.contains(&tex_id_2), "Missing tex_id_2");
        assert!(ids.contains(&tex_id_neighbor), "Missing tex_id_neighbor");
        assert!(ids.contains(&DEFAULT_TEXTURE_ID), "Missing default texture");
        assert_eq!(ids.len(), 4, "Expected 4 unique texture IDs");
    }

    use std::marker::PhantomData;

    use bevy::asset::uuid::Uuid;
    use esm_parser_lib::Landscape;

    use crate::initial_esm_parse::run_initial_parse;

    use super::*;

    pub fn create_test_all_landscapes_for_id_scan(
        cell_configs: Vec<((i32, i32), Landscape)>,
    ) -> AllLandscapes {
        let mut cells = HashMap::new();
        for ((x, y), landscape_data) in cell_configs {
            cells.insert((x, y), landscape_data);
        }
        // Ensure surrounding cells that *might* be accessed by the current sampling logic exist
        // Based on current logic: target_cell (0,0) might access (-1,0)
        let default_textures =
            [DEFAULT_TEXTURE_ID; LAND_TEXTURE_SQUARES_PER_SIDE * LAND_TEXTURE_SQUARES_PER_SIDE];
        let mut default_landscape = Landscape::default();
        default_landscape.grid_x = 0;
        default_landscape.grid_y = 0;
        default_landscape.texture_indices = Some(Box::new(default_textures));

        let mut all_coords_to_check: HashSet<(i32, i32)> = HashSet::new();
        for ((cx, cy), _) in cells.iter() {
            // For a target cell (cx, cy), the sampling logic might look at (cx-1, cy).
            // It also iterates around sampling_origin_cell_x/y up to overall_max_cell_x/y.
            // To be safe for tests, ensure the target cell and its direct "left" neighbor exist.
            all_coords_to_check.insert((*cx, *cy));
            all_coords_to_check.insert((cx - 1, *cy));
            // If sampling were fully symmetric (min_col also had -1), we'd add more:
            // all_coords_to_check.insert((cx,cy-1));
            // all_coords_to_check.insert((cx-1,cy-1));
        }
        for (cx, cy) in all_coords_to_check {
            cells.entry((cx, cy)).or_insert_with(|| {
                let mut l = default_landscape.clone();
                l.grid_x = cx;
                l.grid_y = cy;
                l
            });
        }
        AllLandscapes(cells)
    }
    #[test]
    fn test_get_required_ids_default_only_from_neighbor() {
        let tex_id_1 = 10u16;

        let mut landscape_map = HashMap::new();
        // Cell (0,0) uses only tex_id_1
        let mut landscape = Landscape::default();
        landscape.grid_x = 0;
        landscape.grid_y = 0;
        landscape.texture_indices = Some(Box::new(
            [tex_id_1; LAND_TEXTURE_SQUARES_PER_SIDE * LAND_TEXTURE_SQUARES_PER_SIDE],
        ));
        landscape_map.insert((0, 0), landscape);
        // Neighbor cell (-1,0) uses only DEFAULT_TEXTURE_ID
        let mut landscape = Landscape::default();
        landscape.grid_x = 0;
        landscape.grid_y = 0;
        landscape.texture_indices = Some(Box::new(
            [DEFAULT_TEXTURE_ID; LAND_TEXTURE_SQUARES_PER_SIDE * LAND_TEXTURE_SQUARES_PER_SIDE],
        ));
        landscape_map.insert((-1, 0), landscape);
        let all_landscapes =
            create_test_all_landscapes_for_id_scan(landscape_map.into_iter().collect());

        let result = get_required_texture_ids_for_cell_blendmap(&all_landscapes, 0, 0);
        assert!(result.is_ok());
        let ids = result.unwrap();

        // DEFAULT_TEXTURE_ID is explicitly skipped by the collector.
        // So only tex_id_1 should be present.
        assert!(ids.contains(&tex_id_1));
        assert_eq!(ids.len(), 2);
    }

    // Constants from your setup
    // Assuming these are defined in the parent module or here for test context
    // If not in parent, define them here:
    // const LAND_TEXTURE_SQUARES_PER_SIDE: usize = 16;
    // const LAND_NUM_TEXTURES: usize = LAND_TEXTURE_SQUARES_PER_SIDE * LAND_TEXTURE_SQUARES_PER_SIDE;
    // const DEFAULT_TEXTURE_ID: u16 = 0;

    // Mock Handle for testing if not in a full Bevy app context
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct MockHandle<T> {
        // Using this name to avoid conflict if Bevy Handle is also in scope directly
        id: u32,
        _marker: std::marker::PhantomData<T>,
    }
    impl<T> MockHandle<T>
    where
        T: Asset,
    {
        fn new_dummy(id: u32) -> Handle<T> {
            // Create a deterministic UUID from the u32 for testing
            let uuid = Uuid::from_u128(id as u128);

            // Construct a Handle::Uuid for type T
            Handle::Uuid {
                0: uuid,
                1: PhantomData::<fn() -> T>, // <- PhantomData tells Rust this handle is for T
            }
        }
    }

    // Test helper to create AllLandscapes
    // Ensure Landscape struct has `impl Default` and `impl Clone`
    // and its fields (grid_x, grid_y, texture_indices) are pub or accessible.
    pub fn setup_test_all_landscapes(
        // Made pub if your tests are in a submodule and need access
        cell_configs: Vec<((i32, i32), Landscape)>,
    ) -> AllLandscapes {
        let mut cells = HashMap::new();
        for ((x, y), mut landscape_data) in cell_configs {
            landscape_data.grid_x = x; // Ensure grid coordinates are set
            landscape_data.grid_y = y;
            cells.insert((x, y), landscape_data);
        }

        let default_texture_array = [DEFAULT_TEXTURE_ID; LAND_NUM_TEXTURES];
        let mut default_landscape_template = Landscape::default();
        default_landscape_template.texture_indices = Some(Box::new(default_texture_array));

        let mut all_coords_to_populate: HashSet<(i32, i32)> = HashSet::new();
        for ((cx, cy), _) in cells.iter() {
            all_coords_to_populate.insert((*cx, *cy));
            // Based on current asymmetric sampling (looks left due to row -1 offset)
            all_coords_to_populate.insert((cx - 1, *cy));
            // If fully symmetric sampling was used, add more:
            // all_coords_to_populate.insert((*cx, cy - 1));
            // all_coords_to_populate.insert((*cx - 1, cy - 1));
        }

        for (cx, cy) in all_coords_to_populate {
            cells.entry((cx, cy)).or_insert_with(|| {
                let mut l = default_landscape_template.clone();
                l.grid_x = cx;
                l.grid_y = cy;
                l
            });
        }
        AllLandscapes(cells)
    }

    #[test]
    fn test_output_dimensions_and_format() {
        let mut landscape_map = HashMap::new();
        let mut landscape = Landscape::default();
        landscape.grid_x = 0;
        landscape.grid_y = 0;
        landscape.texture_indices = Some(Box::new([10u16; LAND_NUM_TEXTURES]));
        landscape_map.insert((0, 0), landscape);
        let all_landscapes = setup_test_all_landscapes(landscape_map.into_iter().collect());

        let image_handles = vec![MockHandle::<Image>::new_dummy(0)];
        let mut ltex_to_slot = HashMap::new();
        ltex_to_slot.insert(10u16, 0);
        let image_scale_factor = 2u32;

        let result = generate_alpha_maps_for_active_layers_new(
            &all_landscapes,
            0,
            0,
            &image_handles,
            &ltex_to_slot,
            image_scale_factor,
        );

        assert!(result.is_ok(), "Result was an error: {:?}", result.err());
        let images = result.unwrap();
        assert_eq!(images.len(), 1);

        let expected_grid_dim = LAND_TEXTURE_SQUARES_PER_SIDE + 1;
        let expected_final_dim = expected_grid_dim as u32 * image_scale_factor;

        let image = &images[0];
        assert_eq!(image.texture_descriptor.size.width, expected_final_dim);
        assert_eq!(image.texture_descriptor.size.height, expected_final_dim);
        assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 1);
        assert_eq!(image.texture_descriptor.dimension, TextureDimension::D2);
        assert_eq!(image.texture_descriptor.format, TextureFormat::R8Unorm);
        assert_eq!(
            image.texture_descriptor.usage,
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST
        );
        // Updated to handle Option<Vec<u8>>
        let image_data = image
            .data
            .as_ref()
            .expect("Image data should be Some after creation");
        assert_eq!(
            image_data.len(),
            (expected_final_dim * expected_final_dim) as usize
        );
    }

    #[test]
    fn test_single_active_layer_content() {
        let ltex_id_active = 10u16;
        let mut landscape_map = HashMap::new();
        let mut landscape = Landscape::default();
        landscape.grid_x = 0;
        landscape.grid_y = 0;
        landscape.texture_indices = Some(Box::new([ltex_id_active; LAND_NUM_TEXTURES]));
        landscape_map.insert((0, 0), landscape);
        let all_landscapes = setup_test_all_landscapes(landscape_map.into_iter().collect());

        let image_handles = vec![MockHandle::<Image>::new_dummy(0)];
        let mut ltex_to_slot = HashMap::new();
        ltex_to_slot.insert(ltex_id_active, 0);

        let result = generate_alpha_maps_for_active_layers_new(
            &all_landscapes,
            0,
            0,
            &image_handles,
            &ltex_to_slot,
            2,
        );

        assert!(result.is_ok(), "Result was an error: {:?}", result.err());
        let images = result.unwrap();
        assert_eq!(images.len(), 1);

        // Updated to handle Option<Vec<u8>>
        let image_data = images[0].data.as_ref().expect("Image data should be Some");
        let total_pixels = image_data.len();
        let opaque_pixels = image_data.iter().filter(|&&p| p == 255).count();

        assert!(
            opaque_pixels
                >= total_pixels - (images[0].texture_descriptor.size.width * 4 * 2) as usize,
            "Expected image to be overwhelmingly opaque. Opaque: {}, Total: {}",
            opaque_pixels,
            total_pixels
        );
    }

    #[test]
    fn test_two_active_layers_split_cell_content() {
        let ltex_id_1 = 10u16; // Assigned to bottom half, slot 0
        let ltex_id_2 = 20u16; // Assigned to top half, slot 1

        // Create a flat array for texture_indices
        let mut textures_array = [ltex_id_1; LAND_NUM_TEXTURES];
        for r in 0..(LAND_TEXTURE_SQUARES_PER_SIDE / 2) {
            // Top half is ltex_id_2
            for c in 0..LAND_TEXTURE_SQUARES_PER_SIDE {
                textures_array[r * LAND_TEXTURE_SQUARES_PER_SIDE + c] = ltex_id_2;
            }
        }

        let mut landscape_map = HashMap::new();
        let mut landscape00 = Landscape::default();
        landscape00.grid_x = 0;
        landscape00.grid_y = 0;
        landscape00.texture_indices = Some(Box::new(textures_array));
        landscape_map.insert((0, 0), landscape00);
        let all_landscapes = setup_test_all_landscapes(landscape_map.into_iter().collect());

        let image_handles = vec![
            MockHandle::<Image>::new_dummy(0), // Slot 0 for ltex_id_1
            MockHandle::<Image>::new_dummy(1), // Slot 1 for ltex_id_2
        ];
        let mut ltex_to_slot = HashMap::new();
        ltex_to_slot.insert(ltex_id_1, 0);
        ltex_to_slot.insert(ltex_id_2, 1);

        let result = generate_alpha_maps_for_active_layers_new(
            &all_landscapes,
            0,
            0,
            &image_handles,
            &ltex_to_slot,
            2,
        );

        assert!(result.is_ok(), "Result was an error: {:?}", result.err());
        let images = result.unwrap();
        assert_eq!(images.len(), 2);

        // Updated to handle Option<Vec<u8>>
        let image0_data = images[0]
            .data
            .as_ref()
            .expect("Image 0 data should be Some");
        let total_pixels0 = image0_data.len();
        let img0_opaque_count = image0_data.iter().filter(|&&p| p == 255).count();
        assert!(
            img0_opaque_count > 0 && img0_opaque_count < total_pixels0,
            "Slot 0 map (ltex_id_1) should have mixed alpha. Opaque: {}, Total: {}",
            img0_opaque_count,
            total_pixels0
        );

        let image1_data = images[1]
            .data
            .as_ref()
            .expect("Image 1 data should be Some");
        let total_pixels1 = image1_data.len();
        let img1_opaque_count = image1_data.iter().filter(|&&p| p == 255).count();
        assert!(
            img1_opaque_count > 0 && img1_opaque_count < total_pixels1,
            "Slot 1 map (ltex_id_2) should have mixed alpha. Opaque: {}, Total: {}",
            img1_opaque_count,
            total_pixels1
        );

        // Verify mutual exclusivity
        for i in 0..total_pixels0 {
            // Assuming total_pixels0 == total_pixels1
            let mut opaque_layers_for_pixel = 0;
            if image0_data[i] == 255 {
                opaque_layers_for_pixel += 1;
            }
            if image1_data[i] == 255 {
                opaque_layers_for_pixel += 1;
            }
            assert!(
                opaque_layers_for_pixel <= 1,
                "Pixel at index {} is opaque in multiple layers",
                i
            );
        }
    }

    #[test]
    fn test_ltex_id_in_cell_not_in_slot_map() {
        let ltex_id_cell_only = 30u16;
        let ltex_id_mapped = 40u16;

        let mut landscape_map = HashMap::new();
        let mut landscape = Landscape::default();
        landscape.grid_x = 0;
        landscape.grid_y = 0;
        landscape.texture_indices = Some(Box::new([ltex_id_cell_only; LAND_NUM_TEXTURES]));
        landscape_map.insert((0, 0), landscape);
        let all_landscapes = setup_test_all_landscapes(landscape_map.into_iter().collect());

        let image_handles = vec![MockHandle::<Image>::new_dummy(0)];
        let mut ltex_to_slot = HashMap::new();
        ltex_to_slot.insert(ltex_id_mapped, 0);

        let result = generate_alpha_maps_for_active_layers_new(
            &all_landscapes,
            0,
            0,
            &image_handles,
            &ltex_to_slot,
            2,
        );

        assert!(result.is_ok(), "Result was an error: {:?}", result.err());
        let images = result.unwrap();
        assert_eq!(images.len(), 1);

        let image_data = images[0].data.as_ref().expect("Image data should be Some");
        assert!(
            image_data.iter().all(|&p| p == 0),
            "Image should be all transparent"
        );
    }

    #[test]
    fn test_empty_image_handles_input() {
        let all_landscapes = setup_test_all_landscapes(vec![]);
        let image_handles: Vec<Handle<Image>> = Vec::new();
        let ltex_to_slot = HashMap::new();

        let result = generate_alpha_maps_for_active_layers_new(
            &all_landscapes,
            0,
            0,
            &image_handles,
            &ltex_to_slot,
            2,
        );

        assert!(result.is_ok(), "Result was an error: {:?}", result.err());
        let images = result.unwrap();
        assert!(
            images.is_empty(),
            "Should return empty Vec<Image> if no image handles (slots) are provided"
        );
    }

    #[test]
    fn test_get_blendmap_local_range_for_p1_x_dim() {
        // Inputs for P1 vertex (alpha grid 0,0), X-dimension sampling, from cell (-1,0)
        let current_cell_axis_coord = -1; // current_sampling_cell_x
        let min_overall_cell_axis_coord_for_sampling_block = -1; // sampling_origin_cell_x
        let texture_quad_dimension = LAND_TEXTURE_SQUARES_PER_SIDE; // 16
        let min_overall_tex_axis_coord_for_chunk = 15; // min_row_tex_coord after adjustment
        let max_overall_tex_axis_coord_for_chunk = 30; // max_row_tex_coord_inclusive

        let (local_begin, local_end_exclusive) = get_blendmap_local_range(
            current_cell_axis_coord,
            min_overall_cell_axis_coord_for_sampling_block,
            texture_quad_dimension,
            min_overall_tex_axis_coord_for_chunk,
            max_overall_tex_axis_coord_for_chunk,
        );
        // Expected: samples local texture row 15 from cell -1. Range is (15, 16)
        assert_eq!(local_begin, 15, "P1 X-dim: local_begin mismatch");
        assert_eq!(
            local_end_exclusive, 16,
            "P1 X-dim: local_end_exclusive mismatch"
        );
    }

    #[test]
    fn test_get_blendmap_local_range_for_p1_y_dim() {
        // Inputs for P1 vertex (alpha grid 0,0), Y-dimension sampling, from cell (-1,0)
        // (actually current_sampling_cell_y is 0)
        let current_cell_axis_coord = 0; // current_sampling_cell_y
        let min_overall_cell_axis_coord_for_sampling_block = 0; // sampling_origin_cell_y
        let texture_quad_dimension = LAND_TEXTURE_SQUARES_PER_SIDE; // 16
        let min_overall_tex_axis_coord_for_chunk = 0; // min_col_tex_coord
        let max_overall_tex_axis_coord_for_chunk = 15; // max_col_tex_coord_inclusive

        let (local_begin, local_end_exclusive) = get_blendmap_local_range(
            current_cell_axis_coord,
            min_overall_cell_axis_coord_for_sampling_block,
            texture_quad_dimension,
            min_overall_tex_axis_coord_for_chunk,
            max_overall_tex_axis_coord_for_chunk,
        );
        // Expected: samples local texture cols 0-15 from cell 0. Range is (0, 16)
        assert_eq!(local_begin, 0, "P1 Y-dim: local_begin mismatch");
        assert_eq!(
            local_end_exclusive, 16,
            "P1 Y-dim: local_end_exclusive mismatch"
        );
    }

    #[test]
    fn test_first_cell_sample_generated_for_target_cell_0_0() {
        let mut collected_samples: Vec<CellSample> = Vec::new();
        let sample_handler = |sample: CellSample| {
            collected_samples.push(sample);
        };

        let result = sample_textures_for_single_cell_grid(
            0, // target_cell_x
            0, // target_cell_y
            LAND_TEXTURE_SQUARES_PER_SIDE,
            sample_handler,
        );
        assert!(
            result.is_ok(),
            "sample_textures_for_single_cell_grid failed"
        );
        assert!(!collected_samples.is_empty(), "No samples were collected");

        let first_sample = collected_samples[0];
        let expected_first_sample = CellSample {
            cell_x: -1,          // Sampled from cell to the left
            cell_y: 0,           // Sampled from current target cell's y-row of cells
            src_row_in_cell: 15, // Local X: last texture row in cell -1
            src_col_in_cell: 0,  // Local Y: first texture col in cell -1 (or 0 rather)
            dst_row_on_grid: 0,  // Corresponds to x_grid_idx = 0
            dst_col_on_grid: 0,  // Corresponds to y_grid_idx = 0
        };
        assert_eq!(
            first_sample, expected_first_sample,
            "Mismatch in the first generated CellSample for P1"
        );
    }
    fn check_pixel_block(
        image: &Image,
        expected_alpha: u8,
        x_grid_idx: usize, // 0-16 (dst_row_on_grid / x_idx in generation loop)
        y_grid_idx: usize, // 0-16 (dst_col_on_grid / y_idx in generation loop)
        blendmap_grid_dim: usize, // e.g., 17
        image_scale_factor: u32,
        label: &str,
    ) {
        let final_alpha_map_dim = blendmap_grid_dim as u32 * image_scale_factor;
        let image_data = image
            .data
            .as_ref()
            .unwrap_or_else(|| panic!("Image data is None for {}", label));

        let real_y_base = (blendmap_grid_dim - 1 - y_grid_idx) * image_scale_factor as usize;
        let real_x_base = x_grid_idx * image_scale_factor as usize;

        for y_offset in 0..image_scale_factor as usize {
            for x_offset in 0..image_scale_factor as usize {
                let pixel_y = real_y_base + y_offset;
                let pixel_x = real_x_base + x_offset;
                let flat_idx = pixel_y * final_alpha_map_dim as usize + pixel_x;
                assert!(
                    flat_idx < image_data.len(),
                    "Pixel index out of bounds for {}: flat_idx {}, len {}",
                    label,
                    flat_idx,
                    image_data.len()
                );
                assert_eq!(
                    image_data[flat_idx],
                    expected_alpha,
                    "Pixel mismatch for {} at alpha grid ({}, {}), img pixel ({}, {}), flat_idx {}. Expected {}, Got {}",
                    label,
                    x_grid_idx,
                    y_grid_idx,
                    pixel_x,
                    pixel_y,
                    flat_idx,
                    expected_alpha,
                    image_data[flat_idx]
                );
            }
        }
    }

    #[test]
    fn test_alternating_patterns_and_boundary_sampling_focused_on_p1() {
        let tex_a = 10u16; // Target cell (0,0), top half
        let tex_b = 20u16; // Target cell (0,0), bottom half
        let tex_c = 30u16; // Neighbor cell (-1,0), top half
        let tex_d = 40u16; // Neighbor cell (-1,0), bottom half
        let tex_e = 50u16; // Neighbor cell (0,1), uniform (for P5, though not the focus here)

        // --- Setup Cell (0,0) ---
        let mut cell00_textures = [DEFAULT_TEXTURE_ID; LAND_NUM_TEXTURES];
        for r_idx in 0..LAND_TEXTURE_SQUARES_PER_SIDE {
            // Y
            for c_idx in 0..LAND_TEXTURE_SQUARES_PER_SIDE {
                // X
                let tex_id = if r_idx < LAND_TEXTURE_SQUARES_PER_SIDE / 2 {
                    tex_a
                } else {
                    tex_b
                };
                cell00_textures[r_idx * LAND_TEXTURE_SQUARES_PER_SIDE + c_idx] = tex_id;
            }
        }
        let landscape00 = Landscape {
            grid_x: 0,
            grid_y: 0,
            texture_indices: Some(Box::new(cell00_textures)),
            ..Default::default()
        };

        // --- Setup Cell (-1,0) ---
        let mut cell_neg1_0_textures = [DEFAULT_TEXTURE_ID; LAND_NUM_TEXTURES];
        for r_idx in 0..LAND_TEXTURE_SQUARES_PER_SIDE {
            // Y
            for c_idx in 0..LAND_TEXTURE_SQUARES_PER_SIDE {
                // X
                let tex_id = if r_idx < LAND_TEXTURE_SQUARES_PER_SIDE / 2 {
                    tex_c
                } else {
                    tex_d
                };
                cell_neg1_0_textures[r_idx * LAND_TEXTURE_SQUARES_PER_SIDE + c_idx] = tex_id;
            }
        }
        let landscape_neg1_0 = Landscape {
            grid_x: -1,
            grid_y: 0,
            texture_indices: Some(Box::new(cell_neg1_0_textures)),
            ..Default::default()
        };

        // Cell (0,1) - needed for P5, but also good to have for general neighbor coverage
        let cell01_textures = [tex_e; LAND_NUM_TEXTURES];
        let landscape01 = Landscape {
            grid_x: 0,
            grid_y: 1,
            texture_indices: Some(Box::new(cell01_textures)),
            ..Default::default()
        };

        let mut landscape_map_configs = Vec::new();
        landscape_map_configs.push(((0, 0), landscape00));
        landscape_map_configs.push(((-1, 0), landscape_neg1_0));
        landscape_map_configs.push(((0, 1), landscape01)); // Add cell (0,1) for P5 later
        let all_landscapes = setup_test_all_landscapes(landscape_map_configs);

        // We need 5 slots if we were testing all points P1-P5 as before.
        // For just P1, we mainly care about slots for tex_a, tex_b, tex_c.
        let image_handles = vec![
            MockHandle::<Image>::new_dummy(0), // Slot 0 for default_texture
            MockHandle::<Image>::new_dummy(1), // Slot 1 for tex_a
            MockHandle::<Image>::new_dummy(2), // Slot 2 for tex_b (not directly for P1)
            MockHandle::<Image>::new_dummy(3), // Slot 3 for tex_c (expected for P1)
            MockHandle::<Image>::new_dummy(4), // Slot 4 for tex_d (not directly for P1)
            MockHandle::<Image>::new_dummy(5), // Slot 5 for tex_e (not for P1)
        ];
        let mut ltex_to_slot = HashMap::new();
        ltex_to_slot.insert(DEFAULT_TEXTURE_ID, 0);
        ltex_to_slot.insert(tex_a, 1);
        ltex_to_slot.insert(tex_b, 2);
        ltex_to_slot.insert(tex_c, 3);
        ltex_to_slot.insert(tex_d, 4);
        ltex_to_slot.insert(tex_e, 5);

        let image_scale_factor = 2u32;
        let result = generate_alpha_maps_for_active_layers_new(
            &all_landscapes,
            0,
            0, // Target cell (0,0)
            &image_handles,
            &ltex_to_slot,
            image_scale_factor,
        );

        assert!(
            result.is_ok(),
            "generate_alpha_maps_for_active_layers failed: {:?}",
            result.err()
        );
        let images = result.unwrap();
        assert_eq!(images.len(), 6);

        let blendmap_grid_dim = LAND_TEXTURE_SQUARES_PER_SIDE + 1;

        // --- Verification for P1: Alpha grid vertex (x_grid=0, y_grid=0) ---
        // Expects: cell (-1,0), local tex(x_src=15, y_src=0) -> tex_c (slot 2)
        // This is the TOP-LEFT corner of the alpha map for cell (0,0).
        let (p1_x_grid, p1_y_grid) = (0, 0);
        let p1_expected_ltex_id = tex_c;
        let p1_expected_slot = ltex_to_slot[&p1_expected_ltex_id]; // Should be 3

        // Before checking pixels, let's see what ID was actually sampled for P1
        // This requires access to `sampled_grid_texture_ids` which is internal to the function.
        // For this test, we rely on the output images.

        println!(
            "Checking P1 (alpha_grid_x=0, alpha_grid_y=0): Expected LTex ID {} (Slot {})",
            p1_expected_ltex_id, p1_expected_slot
        );

        for (slot_idx, image) in images.iter().enumerate() {
            let expected_alpha_for_this_layer_at_p1 =
                if slot_idx == p1_expected_slot { 255 } else { 0 };
            println!(
                "  For P1, checking Layer {} (maps to LTex ID if known): Expected alpha {}",
                slot_idx, expected_alpha_for_this_layer_at_p1
            );
            check_pixel_block(
                image,
                expected_alpha_for_this_layer_at_p1,
                p1_x_grid, // x-coordinate on the conceptual 17x17 alpha grid
                p1_y_grid, // y-coordinate on the conceptual 17x17 alpha grid
                blendmap_grid_dim,
                image_scale_factor,
                &format!("P1 (Top-Left) Layer {}", slot_idx),
            );
        }
        println!("Finished checks for P1.");

        let (p5_x_grid, p5_y_grid) = (16, 16); // Bottom-right vertex
        let p5_expected_ltex_id = tex_e; // Samples cell (0,1), src(x=15,y=0)
        let p5_expected_slot = ltex_to_slot[&p5_expected_ltex_id];
        for (slot_idx, image) in images.iter().enumerate() {
            check_pixel_block(
                image,
                if slot_idx == p5_expected_slot { 255 } else { 0 },
                p5_x_grid,
                p5_y_grid,
                blendmap_grid_dim,
                image_scale_factor,
                &format!("P5 Layer {}", slot_idx),
            );
        }
    }
    fn create_alternating_landscape(
        grid_x: i32,
        grid_y: i32,
        tex_upper_half: u16,
        tex_lower_half: u16,
    ) -> Landscape {
        let mut textures = [DEFAULT_TEXTURE_ID; LAND_NUM_TEXTURES];
        for r_idx in 0..LAND_TEXTURE_SQUARES_PER_SIDE {
            // Y
            for c_idx in 0..LAND_TEXTURE_SQUARES_PER_SIDE {
                // X
                let tex_id = if r_idx < LAND_TEXTURE_SQUARES_PER_SIDE / 2 {
                    tex_upper_half
                } else {
                    tex_lower_half
                };
                textures[r_idx * LAND_TEXTURE_SQUARES_PER_SIDE + c_idx] = tex_id;
            }
        }
        Landscape {
            grid_x,
            grid_y,
            texture_indices: Some(Box::new(textures)),
            ..Default::default()
        }
    }
    pub fn setup_test_all_landscapes_comprehensive(
        cell_configs: Vec<((i32, i32), Landscape)>,
        target_cell_x: i32,
        target_cell_y: i32,
    ) -> AllLandscapes {
        let mut cells = HashMap::new();
        for ((x, y), mut landscape_data) in cell_configs {
            landscape_data.grid_x = x;
            landscape_data.grid_y = y;
            cells.insert((x, y), landscape_data);
        }

        let default_texture_array = [DEFAULT_TEXTURE_ID; LAND_NUM_TEXTURES];
        let mut default_landscape_template = Landscape::default();
        default_landscape_template.texture_indices = Some(Box::new(default_texture_array));

        let mut coords_to_ensure: HashSet<(i32, i32)> = HashSet::new();
        // Add target and its direct orthogonal and diagonal neighbors
        for dx in -1..=1 {
            for dy in -1..=1 {
                coords_to_ensure.insert((target_cell_x + dx, target_cell_y + dy));
            }
        }
        // Also ensure neighbors of explicitly configured cells are present
        for ((cx, cy), _) in cells.iter() {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    coords_to_ensure.insert((cx + dx, cy + dy));
                }
            }
        }

        for (cx, cy) in coords_to_ensure {
            cells.entry((cx, cy)).or_insert_with(|| {
                let mut l = default_landscape_template.clone();
                l.grid_x = cx;
                l.grid_y = cy;
                l
            });
        }
        AllLandscapes(cells)
    }
    #[test]
    fn test_center_solid_surrounded_by_alternating_alpha_maps() {
        let tex_main = 100u16;

        let tex_l_top = 10u16;
        let tex_l_bottom = 11u16;
        let tex_r_top = 20u16;
        let tex_r_bottom = 21u16;
        let tex_a_top = 30u16;
        let tex_a_bottom = 31u16; // "Above" (y=-1)
        let tex_b_top = 40u16;
        let tex_b_bottom = 41u16; // "Below" (y=1)

        let target_cell_x = 0;
        let target_cell_y = 0;

        let mut landscape_map_configs = Vec::new();
        // Center cell (0,0) - Solid
        let landscape_main = Landscape {
            grid_x: target_cell_x,
            grid_y: target_cell_y,
            texture_indices: Some(Box::new([tex_main; LAND_NUM_TEXTURES])),
            ..Default::default()
        };
        landscape_map_configs.push(((target_cell_x, target_cell_y), landscape_main));

        // Neighboring cells
        landscape_map_configs.push((
            (target_cell_x - 1, target_cell_y),
            create_alternating_landscape(target_cell_x - 1, target_cell_y, tex_l_top, tex_l_bottom),
        ));
        landscape_map_configs.push((
            (target_cell_x + 1, target_cell_y),
            create_alternating_landscape(target_cell_x + 1, target_cell_y, tex_r_top, tex_r_bottom),
        ));
        landscape_map_configs.push((
            (target_cell_x, target_cell_y - 1),
            create_alternating_landscape(target_cell_x, target_cell_y - 1, tex_a_top, tex_a_bottom),
        ));
        landscape_map_configs.push((
            (target_cell_x, target_cell_y + 1),
            create_alternating_landscape(target_cell_x, target_cell_y + 1, tex_b_top, tex_b_bottom),
        ));

        let all_landscapes = setup_test_all_landscapes_comprehensive(
            landscape_map_configs,
            target_cell_x,
            target_cell_y,
        );

        let required_texture_ids = get_required_texture_ids_for_cell_blendmap(
            &all_landscapes,
            target_cell_x,
            target_cell_y,
        )
        .unwrap();

        let mut ltex_to_slot = HashMap::new();
        let mut handles = Vec::new();
        for (index, texture_id) in required_texture_ids.iter().enumerate() {
            handles.push(MockHandle::<Image>::new_dummy(*texture_id as u32));
            ltex_to_slot.insert(*texture_id as u16, index);
        }

        let image_scale_factor = 2u32;
        let result = generate_alpha_maps_for_active_layers_new(
            // Assuming this is your updated function name
            &all_landscapes,
            target_cell_x,
            target_cell_y,
            &handles,
            &ltex_to_slot,
            image_scale_factor,
        );

        assert!(
            result.is_ok(),
            "generate_alpha_maps failed: {:?}",
            result.err()
        );
        let images = result.unwrap();
        assert_eq!(
            images.len(),
            required_texture_ids.len(),
            "Number of images should match mapped textures"
        );

        let blendmap_grid_dim = LAND_TEXTURE_SQUARES_PER_SIDE + 1;

        // P1: Top-Left vertex of (0,0)'s alpha map (visually bottom-left in image data)
        // Samples cell (-1,0) at its local src(x=15, y=0) -> tex_l_top
        let (p1_x, p1_y) = (0, 0);
        let p1_expected_slot = ltex_to_slot[&tex_l_top];
        for (s_idx, img) in images.iter().enumerate() {
            check_pixel_block(
                img,
                if s_idx == p1_expected_slot { 255 } else { 0 },
                p1_x,
                p1_y,
                blendmap_grid_dim,
                image_scale_factor,
                &format!("P1 Slot {}", s_idx),
            );
        }

        // P2: Top-Right vertex of (0,0)'s alpha map (visually bottom-right)
        // Samples cell (0,0) at its local src(x=15, y=0) -> tex_main
        let (p2_x, p2_y) = (blendmap_grid_dim - 1, 0); // (16,0)
        let p2_expected_slot = ltex_to_slot[&tex_main];
        for (s_idx, img) in images.iter().enumerate() {
            check_pixel_block(
                img,
                if s_idx == p2_expected_slot { 255 } else { 0 },
                p2_x,
                p2_y,
                blendmap_grid_dim,
                image_scale_factor,
                &format!("P2 Slot {}", s_idx),
            );
        }

        // P3: Bottom-Left vertex of (0,0)'s alpha map (visually top-left in image data)
        // Samples cell (-1,1) at its local src(x=15, y=0). Cell (-1,1) is DEFAULT_TEXTURE_ID by helper.
        let (p3_x, p3_y) = (0, blendmap_grid_dim - 1); // (0,16)
        let p3_expected_slot = ltex_to_slot[&DEFAULT_TEXTURE_ID]; // DEFAULT_TEXTURE_ID now has a slot
        for (s_idx, img) in images.iter().enumerate() {
            check_pixel_block(
                img,
                if s_idx == p3_expected_slot { 255 } else { 0 }, // Slot for DEFAULT_ID should be 255
                p3_x,
                p3_y,
                blendmap_grid_dim,
                image_scale_factor,
                &format!("P3 Slot {}", s_idx),
            );
        }

        // P4: Bottom-Right vertex of (0,0)'s alpha map (visually top-right in image data)
        // Samples cell (0,1) at its local src(x=15, y=0) -> tex_b_top
        let (p4_x, p4_y) = (blendmap_grid_dim - 1, blendmap_grid_dim - 1); // (16,16)
        let p4_expected_slot = ltex_to_slot[&tex_b_top];
        for (s_idx, img) in images.iter().enumerate() {
            check_pixel_block(
                img,
                if s_idx == p4_expected_slot { 255 } else { 0 },
                p4_x,
                p4_y,
                blendmap_grid_dim,
                image_scale_factor,
                &format!("P4 Slot {}", s_idx),
            );
        }

        // P5: Mid-Left Edge of (0,0)'s alpha map
        // Samples cell (-1,0) at local src(x=15, y=8) -> tex_l_bottom
        let (p5_x, p5_y) = (0, LAND_TEXTURE_SQUARES_PER_SIDE / 2); // (0,8)
        let p5_expected_slot = ltex_to_slot[&tex_l_bottom];
        for (s_idx, img) in images.iter().enumerate() {
            check_pixel_block(
                img,
                if s_idx == p5_expected_slot { 255 } else { 0 },
                p5_x,
                p5_y,
                blendmap_grid_dim,
                image_scale_factor,
                &format!("P5 Slot {}", s_idx),
            );
        }

        // P6: Mid-Top Edge of (0,0)'s alpha map (visually "top" edge of image data)
        // Samples cell (0,1) at local src(x=~7, y=0) -> tex_b_top
        let (p6_x, p6_y) = (LAND_TEXTURE_SQUARES_PER_SIDE / 2, blendmap_grid_dim - 1); // (8,16)
        let p6_expected_slot = ltex_to_slot[&tex_b_top];
        for (s_idx, img) in images.iter().enumerate() {
            check_pixel_block(
                img,
                if s_idx == p6_expected_slot { 255 } else { 0 },
                p6_x,
                p6_y,
                blendmap_grid_dim,
                image_scale_factor,
                &format!("P6 Slot {}", s_idx),
            );
        }
    }
    #[test]
    fn test_outputting_blendmaps() {
        let (_reader, all_landscapes, _land_textures, _weapon) = run_initial_parse().unwrap();
        let mut all_landscapes = AllLandscapes(all_landscapes);
        for ((current_cell_x, current_cell_y), landscape) in all_landscapes.0.iter_mut() {
            //AS A TEST, SET ALL CHUNKS THAT ARE NOT 0,0 TO A CERTAIN TEXTURE
            let x = 20;
            if *current_cell_x != 0 || *current_cell_y != 0 {
                if landscape.texture_indices.is_none() {
                    continue;
                }
                let current_texture_indices = landscape.texture_indices.as_mut().unwrap();
                for texture_index in current_texture_indices.as_mut() {
                    *texture_index = x;
                }
            }
        }
        for target_x in -1..=1 {
            for target_y in -1..=1 {
                let required_texture_ids =
                    get_required_texture_ids_for_cell_blendmap(&all_landscapes, target_x, target_y)
                        .unwrap();
                let mut ltex_to_slot = HashMap::new();
                let mut handles = Vec::new();
                println!(
                    "{:?}",
                    all_landscapes.0.get(&(-1, 0)).unwrap().texture_indices
                );
                for (index, texture_id) in required_texture_ids.iter().enumerate() {
                    handles.push(MockHandle::<Image>::new_dummy(*texture_id as u32));
                    ltex_to_slot.insert(*texture_id as u16, index);
                }
                let image_scale_factor = 2u32;
                let result = generate_alpha_maps_for_active_layers_new(
                    // Assuming this is your updated function name
                    &all_landscapes,
                    target_x,
                    target_y,
                    &handles,
                    &ltex_to_slot,
                    image_scale_factor,
                );
                assert!(result.is_ok());
                let output_alpha_maps = result.unwrap();
                use std::fs::File;
                use std::io::Write;
                for (slot_index, image) in output_alpha_maps.iter().enumerate() {
                    // Try to find the original LTEX ID that maps to this slot for a more descriptive filename
                    let mut original_ltex_id_for_slot: u16 = 0;
                    for (ltex_id, &s_idx) in ltex_to_slot.iter() {
                        if s_idx == slot_index {
                            original_ltex_id_for_slot = *ltex_id;
                            break;
                        }
                    }

                    let filename = format!(
                        "alpha_map_cell_{}_{}_slot_{}_ltex_{}.pgm",
                        target_x, target_y, slot_index, original_ltex_id_for_slot
                    );

                    match File::create(&filename) {
                        Ok(mut file) => {
                            let header = format!(
                                "P5\n{} {}\n255\n",
                                image.texture_descriptor.size.width,
                                image.texture_descriptor.size.height
                            );
                            if let Err(e) = file.write_all(header.as_bytes()) {
                                warn!("Failed to write PGM header for {}: {}", filename, e);
                                continue;
                            }
                            if let Some(data) = image.data.as_ref() {
                                // Use as_ref() here
                                if let Err(e) = file.write_all(data) {
                                    warn!("Failed to write PGM data for {}: {}", filename, e);
                                } else {
                                    println!("Saved alpha map: {}", filename);
                                }
                            } else {
                                warn!("Image data was None for {} (slot {})", filename, slot_index);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to create file {}: {}", filename, e);
                        }
                    }
                }
            }
        }
    }
}
