use crate::terrain_material::{TerrainBlendMaterial, TerrainMaterialParams};
use bevy::{
    asset::{AssetServer, Assets, Handle, RenderAssetUsages},
    image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    log::warn,
    math::Vec3,
    pbr::MeshMaterial3d,
    render::{
        mesh::{Indices, Mesh, Mesh3d, PrimitiveTopology},
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        view::Visibility,
    },
    transform::components::Transform,
};
use bevy_ecs::{
    entity::Entity,
    name::Name,
    query::Added,
    system::{Commands, Query, Res, ResMut},
};
use bevy_rapier3d::prelude::Collider;
use cdda_bevy::client::defs::{
    AllLandTextures, AllLandscapes, EsmReaderResource, LandscapeAwaitingImages, LandscapeLoader,
    MORROWIND_CELL_SIZE, MORROWIND_HEIGHT_SCALE,
};
use esm_parser_lib::{
    LAND_NUM_VERTS, LAND_SIDE_VERTICES, LAND_TEXTURE_SQUARES_PER_SIDE, LandPayloadData, LandTexture,
};
use std::collections::{HashMap, HashSet};

pub fn load_landscape(
    landscape_loader_q: Query<(Entity, &LandscapeLoader), Added<LandscapeLoader>>,
    mut landscapes: ResMut<AllLandscapes>,
    land_textures: ResMut<AllLandTextures>,
    mut reader_res: ResMut<EsmReaderResource>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for (entity, landscape_loader) in landscape_loader_q {
        let reader = &mut reader_res.0;
        let Some(landscape) = landscapes
            .0
            .get_mut(&(landscape_loader.cell_x, landscape_loader.cell_y))
        else {
            panic!("tried to load landscape that doesn't exist in landscapes map!");
        };

        match landscape.load_detailed_data(reader) {
            Ok(l) => l,
            Err(e) => {
                panic!("failed to load detailed landscape data! {}", e);
            }
        };

        let mut textures = Vec::new();
        let unique_texture_indices: HashSet<u16> =
            landscape.texture_indices.unwrap().iter().cloned().collect();
        //subtract 1 from texture_id because morrowind arrays are beginning 1 index rather than 0
        for texture_id in unique_texture_indices.iter() {
            textures.push(land_textures.0.get(&(*texture_id as u32)).unwrap());
        }
        let (image_handles, texture_index_map) =
            setup_image_handles(&textures, &asset_server, unique_texture_indices);
        commands.entity(entity).insert((LandscapeAwaitingImages {
            image_handles,
            texture_index_map,
        },));
    }
}
pub fn setup_landscape_images(
    landscape_awaiting_images_q: Query<(Entity, &LandscapeLoader, &LandscapeAwaitingImages)>,
    mut images: ResMut<Assets<Image>>,
    landscapes: Res<AllLandscapes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut custom_materials: ResMut<Assets<TerrainBlendMaterial>>,
    mut commands: Commands,
) {
    for (entity, landscape_loader, landscape_awaiting_image) in landscape_awaiting_images_q {
        for image_handle in &landscape_awaiting_image.image_handles {
            if images.get(image_handle).is_none() {
                return;
            }
        }

        //if we get here, we have all the images loaded
        let landscape = landscapes
            .0
            .get(&(landscape_loader.cell_x, landscape_loader.cell_y))
            .unwrap();

        //create 1 alpha map per texture that corresponds to areas the texture is "enabled"
        let alpha_map_array = generate_alpha_maps_for_active_layers(
            &landscape,
            &landscape_awaiting_image.image_handles,
            &landscape_awaiting_image.texture_index_map,
            2,
        )
        .unwrap();
        let array_len = alpha_map_array.len();
        let mut alpha_texture_array_image = setup_2d_image_array(
            landscape_awaiting_image.image_handles.len() as u32,
            alpha_map_array,
        );
        alpha_texture_array_image.texture_descriptor.usage =
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
        let alpha_texture_array_h = images.add(alpha_texture_array_image);

        //find the loaded images and assign them to a 2d texture array
        let texture_array_image = setup_terrain_2d_array(landscape_awaiting_image, &images);
        let texture_handle: Handle<Image> = images.add(texture_array_image);
        commands.entity(entity).remove::<LandscapeAwaitingImages>();
        let payload = landscape.detailed_data.clone().unwrap();
        finish_landscape_setup(
            &mut commands,
            &mut meshes,
            &mut custom_materials,
            alpha_texture_array_h,
            texture_handle,
            *payload,
            array_len,
        );
    }
}

fn finish_landscape_setup(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    custom_materials: &mut ResMut<Assets<TerrainBlendMaterial>>,
    alpha_texture_array_h: Handle<Image>,
    texture_handle: Handle<Image>,
    landscape_detailed_data: LandPayloadData,
    image_handles_array_len: usize,
) {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    let (positions_bevy, normals_bevy, uvs_bevy, colors_bevy, indices) =
        construct_landscape_mesh(&landscape_detailed_data, 0, 0).unwrap();
    let positions_vec3_for_collider: Vec<Vec3> = positions_bevy
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
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions_bevy);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals_bevy);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs_bevy);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors_bevy);
    mesh.insert_indices(Indices::U32(indices));
    if let Err(e) = mesh.generate_tangents() {
        warn!("error generating tangents while creating landscape: {}", e);
    }

    let mesh_h = meshes.add(mesh);
    let material_handle = custom_materials.add(TerrainBlendMaterial {
        source_textures: texture_handle,
        alpha_map_array: alpha_texture_array_h,
        params: TerrainMaterialParams {
            num_textures_in_array: image_handles_array_len as u32, // Example: The number of unique textures you put in the array
            diffuse_tiling_factor: 16.0,
        },
    });

    commands.spawn((
        Visibility::Visible,
        Name::new("land"),
        Mesh3d(mesh_h),
        MeshMaterial3d(material_handle),
        Collider::trimesh(positions_vec3_for_collider, indices_vec3_for_collider).unwrap(),
        Transform::from_xyz(2.0, 2.0, -2.0),
    ));
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
fn setup_2d_image_array(len: u32, image_array: Vec<Image>) -> Image {
    //concat all images into 1 u8 vec
    let mut all_alpha_array_pixel_data = Vec::new();
    let mut image_width = 0;
    let mut image_height = 0;
    let mut image_format = TextureFormat::R8Unorm; //just a default, this is modified in the loop

    for image in image_array {
        all_alpha_array_pixel_data.extend_from_slice(&image.data.clone().unwrap());
        image_width = image.width();
        image_height = image.height();
        image_format = image.texture_descriptor.format;
    }
    let alpha_texture_array_image = Image::new(
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
    alpha_texture_array_image
}
fn construct_landscape_mesh(
    payload: &LandPayloadData,
    cell_grid_x: i32,
    cell_grid_y: i32,
) -> Option<(
    Vec<[f32; 3]>, // positions_bevy
    Vec<[f32; 3]>, // normals_bevy
    Vec<[f32; 2]>, // uvs_bevy
    Vec<[f32; 4]>, // original_vclr_bevy (original vertex colors for tinting)
    Vec<u32>,      // indices
)> {
    let vhgt_data = payload.vertex_heights.as_ref()?; // From LAND.VHGT

    // --- 1. Heightmap Generation ---
    let mut unscaled_heights = vec![0.0f32; LAND_NUM_VERTS];
    let mut current_z_at_first_col_unscaled = vhgt_data.height_offset; // Base for the cell

    for y_idx in 0..LAND_SIDE_VERTICES {
        current_z_at_first_col_unscaled +=
            vhgt_data.height_data[y_idx * LAND_SIDE_VERTICES + 0] as f32;
        unscaled_heights[y_idx * LAND_SIDE_VERTICES + 0] = current_z_at_first_col_unscaled;

        // For the rest of the row (columns 1 to 64)
        let mut current_z_in_row_unscaled = current_z_at_first_col_unscaled;
        for x_idx in 1..LAND_SIDE_VERTICES {
            current_z_in_row_unscaled +=
                vhgt_data.height_data[y_idx * LAND_SIDE_VERTICES + x_idx] as f32;
            unscaled_heights[y_idx * LAND_SIDE_VERTICES + x_idx] = current_z_in_row_unscaled;
        }
    }

    let morrowind_vertex_z_values: Vec<f32> = unscaled_heights
        .into_iter()
        .map(|h| h * MORROWIND_HEIGHT_SCALE)
        .collect();

    let mut positions_bevy: Vec<[f32; 3]> = Vec::with_capacity(LAND_NUM_VERTS);
    let mut normals_bevy: Vec<[f32; 3]> = Vec::with_capacity(LAND_NUM_VERTS);
    let mut uvs_bevy: Vec<[f32; 2]> = Vec::with_capacity(LAND_NUM_VERTS);
    let mut original_vclr_bevy: Vec<[f32; 4]> = Vec::with_capacity(LAND_NUM_VERTS);

    let cell_mw_origin_x = cell_grid_x as f32 * MORROWIND_CELL_SIZE;
    let cell_mw_origin_y = cell_grid_y as f32 * MORROWIND_CELL_SIZE; // This is Morrowind's Y-plane

    for y_idx_mesh in 0..LAND_SIDE_VERTICES {
        // Mesh row index: 0 to 64
        for x_idx_mesh in 0..LAND_SIDE_VERTICES {
            // Mesh col index: 0 to 64
            let vertex_index_flat = y_idx_mesh * LAND_SIDE_VERTICES + x_idx_mesh;

            // --- 2. Vertex Position Generation ---
            // Morrowind X and Y (horizontal plane) coordinates
            let mw_x_pos = cell_mw_origin_x
                + (x_idx_mesh as f32 / (LAND_SIDE_VERTICES - 1) as f32 * MORROWIND_CELL_SIZE);
            let mw_y_pos = cell_mw_origin_y // This is MW's Y on the horizontal plane
                + (y_idx_mesh as f32 / (LAND_SIDE_VERTICES - 1) as f32 * MORROWIND_CELL_SIZE);
            let mw_z_pos = morrowind_vertex_z_values[vertex_index_flat]; // This is MW's Z (height)

            positions_bevy.push([-mw_x_pos, mw_z_pos, mw_y_pos]); // Bevy Y is Up (MW Z), Bevy Z is negated MW Y

            // --- 3. Normal Data Processing ---
            if let Some(normals_data) = payload.vertex_normals.as_ref() {
                // From LAND.VNML
                let esm_normal = normals_data[vertex_index_flat];
                let n_mw_x = esm_normal.x as f32 / 127.0;
                let n_mw_y = esm_normal.y as f32 / 127.0; // MW Y-component of normal
                let n_mw_z = esm_normal.z as f32 / 127.0; // MW Z-component of normal (up)

                // Convert normal to Bevy's coordinate system
                // BevyX = MWX, BevyY = MWZ, BevyZ = -MWY
                // Assuming Vec3 is glam::Vec3 or similar
                let bevy_normal = glam::Vec3::new(-n_mw_x, n_mw_z, n_mw_y).normalize_or_zero();
                normals_bevy.push(bevy_normal.into());
            } else {
                // Default normal (Y-up in Bevy) if VNML data is not available
                // Note: Ideally, calculate from heightmap if VNML is missing.
                normals_bevy.push([0.0, 1.0, 0.0]);
            }
            // --- 4. Color Data Processing ---
            if let Some(colors_data) = payload.vertex_colors.as_ref() {
                // From LAND.VCLR
                let esm_color = colors_data[vertex_index_flat];
                original_vclr_bevy.push([
                    esm_color.r as f32 / 255.0,
                    esm_color.g as f32 / 255.0,
                    esm_color.b as f32 / 255.0,
                    1.0, // Alpha
                ]);
            } else {
                original_vclr_bevy.push([1.0, 1.0, 1.0, 1.0]); // Default white
            }

            // --- 5. UV Generation (with V flipped) ---
            let norm_u_mesh = x_idx_mesh as f32 / (LAND_SIDE_VERTICES - 1) as f32;
            let norm_v_mesh = y_idx_mesh as f32 / (LAND_SIDE_VERTICES - 1) as f32;
            uvs_bevy.push([norm_u_mesh, 1.0 - norm_v_mesh]); // flipped V of uv
        }
    }

    // --- 6. Index Buffer Generation (Diamond Pattern) ---
    // (without LOD stitching).
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
                // Diamond pattern condition
                // Triangle 1: B, D, C (This is CCW for top face: (qx+1,qy) -> (qx+1,qy+1) -> (qx,qy+1))
                indices_list.push(v_b);
                indices_list.push(v_d);
                indices_list.push(v_c);
                // Triangle 2: A, B, C (This is CCW for top face: (qx,qy) -> (qx+1,qy) -> (qx,qy+1))
                indices_list.push(v_a);
                indices_list.push(v_b);
                indices_list.push(v_c);
            } else {
                // Triangle 1: A, D, C (This is CCW for top face: (qx,qy) -> (qx+1,qy+1) -> (qx,qy+1))
                indices_list.push(v_a);
                indices_list.push(v_d);
                indices_list.push(v_c);
                // Triangle 2: A, B, D (This is CCW for top face: (qx,qy) -> (qx+1,qy) -> (qx+1,qy+1))
                indices_list.push(v_a);
                indices_list.push(v_b);
                indices_list.push(v_d);
            }
        }
    }
    Some((
        positions_bevy,
        normals_bevy,
        uvs_bevy,
        original_vclr_bevy,
        indices_list,
    ))
}
pub fn setup_image_handles(
    textures: &Vec<&LandTexture>,
    asset_server: &Res<AssetServer>,
    indices: HashSet<u16>,
) -> (Vec<Handle<Image>>, HashMap<u16, usize>) {
    let mut image_handles = Vec::new();
    let mut ltex_id_to_slot_index_map = HashMap::new();
    for texture in textures {
        if indices.contains(&(texture.index as u16)) {
            let texture_path = format!("textures/{}", texture.texture_file);
            println!("texture path: {} {}", texture_path, texture.index);
            image_handles.push(asset_server.load(texture_path));
            ltex_id_to_slot_index_map.insert(texture.index as u16, image_handles.len() - 1);
        }
    }
    (image_handles, ltex_id_to_slot_index_map)
}

use esm_parser_lib::Landscape;

pub fn generate_alpha_maps_for_active_layers(
    landscape: &Landscape,
    image_handles_for_cell: &Vec<Handle<Image>>, // These are the diffuse textures for the cell
    ltex_id_to_slot_index_map: &HashMap<u16, usize>, // Maps LTEX ID to slot in image_handles_for_cell
    image_scale_factor: u32,
) -> Result<Vec<Image>, String> {
    if image_scale_factor == 0 {
        return Err("image_scale_factor cannot be zero.".to_string());
    }

    let texture_indices_grid_slice = match landscape.texture_indices.as_ref() {
        Some(indices) => indices.as_slice(),
        None => return Err("LandPayloadData does not contain texture_indices.".to_string()),
    };

    let num_active_layers = image_handles_for_cell.len();
    if num_active_layers <= 1 {
        // OpenMW produces no blend maps for 0 or 1 unique texture layer on the cell
        return Ok(Vec::new());
    }

    // base_vtex_grid_dim is the dimension of the VTEX grid (e.g., 16 for a 16x16 VTEX grid)
    let base_vtex_grid_dim: usize = LAND_TEXTURE_SQUARES_PER_SIDE;
    let final_alpha_map_dim = base_vtex_grid_dim as u32 * image_scale_factor;

    let mut output_alpha_map_images: Vec<Image> = Vec::with_capacity(num_active_layers);

    // Create blank (all zeros) alpha map images for each active layer
    for _i in 0..num_active_layers {
        let initial_pixel_data = vec![0u8; (final_alpha_map_dim * final_alpha_map_dim) as usize];
        let mut alpha_map_image = Image::new(
            Extent3d {
                width: final_alpha_map_dim,
                height: final_alpha_map_dim,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            initial_pixel_data,              // Provide the zeroed data directly
            TextureFormat::R8Unorm,          // Single channel (alpha/grayscale)
            RenderAssetUsages::RENDER_WORLD, // Default Bevy 0.13 asset usage
        );
        // Ensure the texture can be used in bindings (for shaders)
        alpha_map_image.texture_descriptor.usage =
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST; // COPY_DST if you might update it later
        output_alpha_map_images.push(alpha_map_image);
    }

    // Populate the alpha map pixel data by iterating over the 16x16 VTEX grid cells
    for y_vtex_cell in 0..base_vtex_grid_dim {
        // 0 to 15 (VTEX grid y-coordinate)
        for x_vtex_cell in 0..base_vtex_grid_dim {
            // 0 to 15 (VTEX grid x-coordinate)
            let vtex_grid_linear_idx = y_vtex_cell * base_vtex_grid_dim + x_vtex_cell;
            let raw_ltex_id = texture_indices_grid_slice[vtex_grid_linear_idx];

            // Find which active layer (slot) this VTEX ID corresponds to
            if let Some(&layer_slot_index) = ltex_id_to_slot_index_map.get(&raw_ltex_id) {
                // Ensure the slot index is valid for the output_alpha_map_images vector
                if layer_slot_index < num_active_layers {
                    // Get a mutable reference to the pixel data of the correct alpha map image
                    let current_alpha_map_image_data: &mut [u8] = output_alpha_map_images
                        [layer_slot_index]
                        .data
                        .as_mut()
                        .unwrap();

                    // Determine the top-left corner of the block to fill in the target alpha map image
                    // Apply V-flip for y-coordinate to match OpenMW's blend map generation for consistency
                    //
                    let flipped_y_vtex_cell = (base_vtex_grid_dim - 1) - y_vtex_cell;
                    let block_start_y_in_alpha_map =
                        flipped_y_vtex_cell as u32 * image_scale_factor;
                    let block_start_x_in_alpha_map = x_vtex_cell as u32 * image_scale_factor;

                    // Fill the (image_scale_factor x image_scale_factor) block of pixels
                    for y_offset in 0..image_scale_factor {
                        for x_offset in 0..image_scale_factor {
                            let pixel_y_in_alpha_map = block_start_y_in_alpha_map + y_offset;
                            let pixel_x_in_alpha_map = block_start_x_in_alpha_map + x_offset;

                            let pixel_linear_idx = (pixel_y_in_alpha_map * final_alpha_map_dim
                                + pixel_x_in_alpha_map)
                                as usize;

                            if pixel_linear_idx < current_alpha_map_image_data.len() {
                                current_alpha_map_image_data[pixel_linear_idx] = 255; // Mark this pixel as opaque for the current layer
                            }
                        }
                    }
                }
            }
            // If raw_ltex_id is 0 (default texture) or not in ltex_id_to_slot_index_map,
            // it means it doesn't correspond to one of the active unique layers for this cell.
            // Its contribution would typically be handled by a base layer in the shader or if no other layers are opaque.
        }
    }
    /* DEBUG
        for (idx, image) in output_alpha_map_images.iter().enumerate() {
            let filename = format!("alpha_map_layer_{}.pgm", idx);
            let width = image.texture_descriptor.size.width;
            let height = image.texture_descriptor.size.height;
            if let Some(data) = &image.data {
                if data.len() == (width * height) as usize {
                    let mut file = File::create(&filename).expect("Failed to create PGM file");
                    write!(file, "P5\n{} {}\n255\n", width, height)
                        .expect("Failed to write PGM header");
                    file.write_all(data)
                        .expect("Failed to write PGM image data");
                    println!("Saved debug image as {}", filename);
                } else {
                    eprintln!(
                        "Data length mismatch for PGM save: {} vs {}",
                        data.len(),
                        width * height
                    );
                }
            }
        }
    */
    Ok(output_alpha_map_images)
}
