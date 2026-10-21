use super::terrain_material::{TerrainBlendMaterial, TerrainMaterialParams};
use crate::{
    const_defs::CELL_SIZE_METERS,
    defs::{ESM_READER, LandTextureDefinitions, LandscapeAwaitingImages},
    landscape::{ALL_LANDSCAPES, LandscapeCell, LandscapeLoader},
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
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
    transform::components::Transform,
};
use either::Either;
use esm_parser_lib::{
    LAND_NUM_TEXTURES, LAND_NUM_VERTS, LAND_SIDE_VERTICES, LAND_TEXTURE_SQUARES_PER_SIDE,
    LandTexture, LandVertexColor, LandVertexHeightData, LandVertexNormal, Landscape,
};
use std::collections::{HashMap, HashSet};
include!(concat!(env!("OUT_DIR"), "/landscape_lut.rs"));

pub fn load_landscape(
    added_event: On<Add<LandscapeLoader>>,
    landscape_loader_q: Query<(Entity, &LandscapeLoader)>,
    mut commands: Commands,
) {
    let Ok((entity, landscape_loader)) = landscape_loader_q.get(added_event.entity) else {
        return;
    };
    let thread_pool = AsyncComputeTaskPool::get();
    let current_cell_x = landscape_loader.cell_x;
    let current_cell_y = landscape_loader.cell_y;
    let task = thread_pool.spawn(async move {
        // We need the detailed data of our neighbors too so we can smooth across the edges
        for dy in -1..=1 {
            for dx in -1..=1 {
                let neighbor = (current_cell_x + dx, current_cell_y + dy);
                blocking_load_detailed_data_if_needed(neighbor.0, neighbor.1);
            }
        }
        let lock = ALL_LANDSCAPES
            .get()
            .expect("ALL_LANDSCAPES not initialized");
        let landscapes = lock.read().unwrap();
        // Get all the unique texture ids we need to draw this cell
        let unique_ltex_ids_for_cell_and_border =
            get_required_texture_ids_for_cell_blendmap(&landscapes, current_cell_x, current_cell_y)
                .unwrap();

        Some(LandscapeImageTaskResult {
            ltex_ids: unique_ltex_ids_for_cell_and_border,
            current_cell: (current_cell_x, current_cell_y),
        })
    });
    commands
        .entity(entity)
        .insert(ComputeLandscapeImageTask(task));
}
struct LandscapeImageTaskResult {
    current_cell: (i32, i32),
    ltex_ids: HashSet<u16>,
}
#[derive(Component)]
pub struct ComputeLandscapeImageTask(Task<Option<LandscapeImageTaskResult>>);
pub fn poll_sampled_landscape_images(
    mut tasks: Query<(Entity, &mut ComputeLandscapeImageTask)>,
    land_texture_definitions: Res<LandTextureDefinitions>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for (landscape_entity, mut task) in &mut tasks {
        if let Some(landscape_image_task_result_opt) = check_ready(&mut task.0) {
            commands
                .entity(landscape_entity)
                .remove::<ComputeLandscapeImageTask>();
            let Some(task_result) = landscape_image_task_result_opt else {
                warn!(
                    "failed to build landscape! despawning entity from sampled_landscape_images."
                );
                commands.entity(landscape_entity).despawn();
                return;
            };
            let mut textures_for_material_setup: Vec<&LandTexture> = Vec::new();
            for ltex_id in task_result.ltex_ids.iter() {
                if let Some(land_texture_def) = land_texture_definitions.0.get(&(*ltex_id as u32)) {
                    textures_for_material_setup.push(land_texture_def);
                } else {
                    warn!(
                        "Missing LandTexture definition for LTEX ID: {} in cell ({},{}) during material setup.",
                        ltex_id, task_result.current_cell.0, task_result.current_cell.1
                    );
                }
            }

            // start loading all the images for these texture ids
            let mut image_handles = Vec::new();
            let mut texture_index_map = HashMap::new();
            for (slot_index, land_texture_def) in textures_for_material_setup.iter().enumerate() {
                let texture_path = format!("Textures/{}", land_texture_def.texture_file);
                // println!("trying to load {}", texture_path);
                image_handles.push(asset_server.load(texture_path));
                texture_index_map.insert(land_texture_def.index as u16, slot_index);
            }

            if image_handles.is_empty() && !task_result.ltex_ids.is_empty() {
                warn!(
                    "Cell ({},{}) ended up with no image handles despite having unique texture IDs. Check LandTexture definitions and asset paths.",
                    task_result.current_cell.0, task_result.current_cell.1
                );
            }
            commands
                .entity(landscape_entity)
                .insert(LandscapeAwaitingImages {
                    image_handles,
                    texture_index_map,
                });
        }
    }
}
struct LandscapeTerrainImagesTaskResult {
    texture_array_image: Image,
    texture_index_map: HashMap<u16, usize>,
    image_handles_len: usize,
}
#[derive(Component)]
pub struct LandscapeTerrainImagesTask(Task<LandscapeTerrainImagesTaskResult>);
pub fn setup_landscape_images(
    landscape_awaiting_images_q: Query<(Entity, &mut LandscapeAwaitingImages)>,
    images: Res<Assets<Image>>,
    mut commands: Commands,
) {
    // check to make sure texture images are loaded
    for (entity, mut landscape_awaiting_image) in landscape_awaiting_images_q {
        for image_handle in &landscape_awaiting_image.image_handles {
            if images.get(image_handle).is_none() && images.contains(image_handle) {
                dbg!("contains but not loaded");
            }
            if images.get(image_handle).is_none() {
                return;
            }
        }
        // if we get here, we have all the images loaded
        let image_handles = landscape_awaiting_image.image_handles.clone();
        let mut texture_images = Vec::new();
        for handle in image_handles {
            if let Some(image) = images.get(&handle) {
                texture_images.push(image.clone());
            }
        }
        let texture_index_map = std::mem::take(&mut landscape_awaiting_image.texture_index_map);
        let image_handles_len = landscape_awaiting_image.image_handles.len();

        // Clone all the images in a background task so we don't hitch the main game
        let thread_pool = AsyncComputeTaskPool::get();
        let task = thread_pool.spawn(async move {
            //find the loaded images and assign them to a 2d texture array
            let texture_array_image = setup_terrain_2d_array(texture_images);
            LandscapeTerrainImagesTaskResult {
                texture_array_image,
                texture_index_map,
                image_handles_len,
            }
        });
        commands
            .entity(entity)
            .insert(LandscapeTerrainImagesTask(task));
        commands.entity(entity).remove::<LandscapeAwaitingImages>();
    }
}
pub fn poll_landscape_textures_array_ready(
    mut tasks: Query<(Entity, &LandscapeLoader, &mut LandscapeTerrainImagesTask)>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) {
    for (landscape_entity, landscape_loader, mut task) in &mut tasks {
        let Some(landscape_task_result) = check_ready(&mut task.0) else {
            return;
        };
        let landscapes = ALL_LANDSCAPES.get().unwrap().read().unwrap();
        let texture_handle: Handle<Image> = images.add(landscape_task_result.texture_array_image);

        drop(landscapes);
        finish_landscape_setup(
            landscape_entity,
            &mut commands,
            landscape_task_result.image_handles_len,
            landscape_task_result.texture_index_map,
            texture_handle,
            (landscape_loader.cell_x, landscape_loader.cell_y),
        );
    }
}
struct LandscapeTaskResult {
    mesh: Mesh,
    collider: Collider,
    texture_handle_2d: Handle<Image>,
    image_handles_array_len: usize,
    alpha_map_array_image: Image,
}
#[derive(Component)]
pub struct ComputeLandscapeTask(Task<Option<LandscapeTaskResult>>);
fn finish_landscape_setup(
    landscape_entity: Entity,
    commands: &mut Commands,
    image_handles_array_len: usize,
    texture_index_map: HashMap<u16, usize>,
    texture_handle_2d: Handle<Image>,
    cell_position: (i32, i32),
) {
    let thread_pool = AsyncComputeTaskPool::get();
    let task = thread_pool.spawn(async move {
        let landscapes_ref = ALL_LANDSCAPES.get().unwrap().read().unwrap();
        let Some((mesh, collider)) =
            construct_landscape_mesh(cell_position.0, cell_position.1, &landscapes_ref)
        else {
            dbg!("failed to build mesh data!");
            return None;
        };

        // create 1 alpha map per texture that corresponds to areas the texture is "enabled"
        let alpha_map_array = match generate_alpha_maps_for_active_layers(
            &landscapes_ref,
            cell_position.0,
            cell_position.1,
            image_handles_array_len,
            texture_index_map,
            2,
        ) {
            Ok(arr) => arr,
            Err(e) => {
                dbg!(e);
                return None;
            }
        };
        let mut alpha_2d_image_array =
            setup_2d_image_array(image_handles_array_len as u32, alpha_map_array);
        alpha_2d_image_array.texture_descriptor.usage =
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
        alpha_2d_image_array.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            ..Default::default()
        });

        Some(LandscapeTaskResult {
            mesh,
            collider,
            image_handles_array_len,
            texture_handle_2d,
            alpha_map_array_image: alpha_2d_image_array,
        })
    });

    commands
        .entity(landscape_entity)
        .insert(ComputeLandscapeTask(task));

    commands.entity(landscape_entity).insert((
        Visibility::Visible,
        Name::new("land"),
        LandscapeCell {
            x: cell_position.0,
            y: cell_position.1,
        },
    ));
    let cell_translation = Vec3::new(
        -cell_position.0 as f32 * CELL_SIZE_METERS,
        0.0,
        cell_position.1 as f32 * CELL_SIZE_METERS,
    );

    commands
        .entity(landscape_entity)
        .remove::<LandscapeLoader>();
    commands
        .entity(landscape_entity)
        .insert(Transform::from_translation(cell_translation));
}
pub fn poll_task_finished(
    mut meshes: ResMut<Assets<Mesh>>,
    mut tasks: Query<(Entity, &mut ComputeLandscapeTask)>,
    mut images: ResMut<Assets<Image>>,
    mut custom_materials: ResMut<Assets<TerrainBlendMaterial>>,
    mut commands: Commands,
) {
    for (landscape_entity, mut task) in &mut tasks {
        if let Some(landscape_task_result_opt) = check_ready(&mut task.0) {
            let Some(landscape_task_result) = landscape_task_result_opt else {
                warn!("failed to build landscape! despawning entity from task_finished.");
                commands.entity(landscape_entity).despawn();
                return;
            };
            let alpha_texture_array_h = images.add(landscape_task_result.alpha_map_array_image);
            let base_vtex_grid_dim = LAND_TEXTURE_SQUARES_PER_SIDE as f32; // 16
            let material_handle = custom_materials.add(TerrainBlendMaterial {
                source_textures: landscape_task_result.texture_handle_2d,
                alpha_map_array: alpha_texture_array_h,
                params: TerrainMaterialParams {
                    num_textures_in_array: landscape_task_result.image_handles_array_len as u32, // The number of unique textures
                    diffuse_tiling_factor: 16.0,
                    base_vtex_grid_dim,
                },
            });
            let mesh_h = meshes.add(landscape_task_result.mesh);

            commands.entity(landscape_entity).insert((
                Mesh3d(mesh_h),
                MeshMaterial3d(material_handle),
                RigidBody::Static,
                landscape_task_result.collider,
            ));
            commands
                .entity(landscape_entity)
                .remove::<ComputeLandscapeTask>();
        }
    }
}
pub fn construct_landscape_mesh(
    cell_grid_x: i32,
    cell_grid_y: i32,
    landscapes: &HashMap<(i32, i32), Landscape>,
) -> Option<(Mesh, Collider)> {
    let current_cell_key = (cell_grid_x, cell_grid_y);
    // Read all the data and populate the heightmap/normals/colors
    let (morrowind_vertex_z_values, initial_esm_normals, initial_esm_colors) = {
        let landscape_entry = landscapes.get(&current_cell_key)?;
        let landscape_detailed_data = landscape_entry.detailed_data.as_ref()?;
        let z_values = landscape_detailed_data
            .vertex_heights
            .as_ref()
            .map(|vhgt| vhgt.heights.clone())
            .unwrap_or_else(|| LandVertexHeightData::default().heights);

        // Extract initial per-vertex normals and colors into new owned Vecs
        let mut temp_normals = Vec::with_capacity(LAND_NUM_VERTS);
        let mut temp_colors = Vec::with_capacity(LAND_NUM_VERTS);

        // Copy normals
        let normal_iter = if let Some(data) = landscape_detailed_data.vertex_normals.as_ref() {
            Either::Left(data.iter().copied())
        } else {
            Either::Right(
                std::iter::repeat(LandVertexNormal { x: 0, y: 0, z: 127 }).take(LAND_NUM_VERTS),
            )
        };
        temp_normals.extend(normal_iter);

        // Copy colors
        let color_iter = if let Some(data) = landscape_detailed_data.vertex_colors.as_ref() {
            Either::Left(data.iter().copied())
        } else {
            Either::Right(
                std::iter::repeat(LandVertexColor {
                    r: 128,
                    g: 128,
                    b: 128,
                })
                .take(LAND_NUM_VERTS),
            )
        };
        temp_colors.extend(color_iter);

        (z_values, temp_normals, temp_colors)
    };

    // Calculate final vertex attributes, performing mutations on `landscapes` as needed
    let mut positions_bevy: Vec<[f32; 3]> = Vec::with_capacity(LAND_NUM_VERTS);
    let mut uvs_bevy: Vec<[f32; 2]> = Vec::with_capacity(LAND_NUM_VERTS);
    let mut final_normals_bevy: Vec<[f32; 3]> = vec![[0.0, 1.0, 0.0]; LAND_NUM_VERTS];
    let mut final_vclr_bevy: Vec<[f32; 4]> = vec![[1.0, 1.0, 1.0, 1.0]; LAND_NUM_VERTS];

    let lx_max_idx = LAND_SIDE_VERTICES - 1;
    // Create a small, fixed-size cache for the 9 possible cell data references
    let mut cell_cache = [None; 9];

    // Loop through the 3x3 grid around the current cell
    for dy in -1..=1 {
        for dx in -1..=1 {
            let key = (cell_grid_x + dx, cell_grid_y + dy);
            // Store the reference to the normals/colors in your cache
            cell_cache[((dy + 1) * 3 + (dx + 1)) as usize] =
                landscapes.get(&key).and_then(|e| e.detailed_data.as_ref());
        }
    }
    let get_cached_normal = |dx: i8, dy: i8, vx: usize, vy: usize| -> Option<LandVertexNormal> {
        let cache_idx = ((dy + 1) * 3 + (dx + 1)) as usize;
        cell_cache[cache_idx]?
            .vertex_normals
            .as_ref()?
            .get(vy * LAND_SIDE_VERTICES + vx)
            .copied()
    };
    let get_cached_color = |dx: i8, dy: i8, vx: usize, vy: usize| -> Option<LandVertexColor> {
        let cache_idx = ((dy + 1) * 3 + (dx + 1)) as usize;
        cell_cache[cache_idx]?
            .vertex_colors
            .as_ref()?
            .get(vy * LAND_SIDE_VERTICES + vx)
            .copied()
    };

    for y_idx_mesh in 0..LAND_SIDE_VERTICES {
        for x_idx_mesh in 0..LAND_SIDE_VERTICES {
            let vertex_index_flat = y_idx_mesh * LAND_SIDE_VERTICES + x_idx_mesh;

            // Look up the relative positions in a lookup table
            let mw_x = RELATIVE_POS_LUT[x_idx_mesh];
            let mw_y = RELATIVE_POS_LUT[y_idx_mesh];
            let mw_z = morrowind_vertex_z_values[vertex_index_flat];
            positions_bevy.push([-mw_x, mw_z, mw_y]);
            let u = UV_LUT[x_idx_mesh];
            let v = UV_LUT[y_idx_mesh];
            uvs_bevy.push([u, 1.0 - v]);

            let esm_normal = initial_esm_normals[vertex_index_flat];
            let mut esm_normal_vec3 = transform_mw_normal_to_bevy(Vec3::new(
                esm_normal.x as f32,
                esm_normal.y as f32,
                esm_normal.z as f32,
            ));
            let mut esm_color_to_use = initial_esm_colors[vertex_index_flat];
            let is_corner = (x_idx_mesh == 0 || x_idx_mesh == lx_max_idx)
                && (y_idx_mesh == 0 || y_idx_mesh == lx_max_idx);
            // 1. Identify boundary vertices (Edges and Corners)
            let is_boundary = x_idx_mesh == 0
                || x_idx_mesh == lx_max_idx
                || y_idx_mesh == 0
                || y_idx_mesh == lx_max_idx;

            if is_boundary {
                let neighbors = &NEIGHBOR_LUT[vertex_index_flat];

                if is_corner {
                    // CORNER LOGIC: Average neighbors to smooth lighting across cell corners
                    let mut sum_mw_normals_vec3 = Vec3::ZERO;
                    let mut count = 0;

                    for info in neighbors {
                        if let Some(n_esm) = get_cached_normal(
                            info.cell_dx as i8,
                            info.cell_dy as i8,
                            info.local_vx,
                            info.local_vy,
                        ) {
                            sum_mw_normals_vec3 += transform_mw_normal_to_bevy(Vec3::new(
                                n_esm.x as f32,
                                n_esm.y as f32,
                                n_esm.z as f32,
                            ));
                            count += 1;
                        }
                    }

                    if count > 0 {
                        let avg_mw_normal =
                            (sum_mw_normals_vec3 / count as f32).normalize_or_zero();
                        esm_normal_vec3 = avg_mw_normal;
                    }
                } else {
                    // Edges: Direct lookup using the LUT to cross cell boundaries
                    // We pick the neighbor corresponding to the edge we are on (Up, Down, Left, or Right)
                    let info = if x_idx_mesh == 0 {
                        &neighbors[2]
                    }
                    // Left
                    else if x_idx_mesh == lx_max_idx {
                        &neighbors[3]
                    }
                    // Right
                    else if y_idx_mesh == 0 {
                        &neighbors[0]
                    }
                    // Up
                    else {
                        &neighbors[1]
                    }; // Down (y_idx_mesh == lx_max_idx)

                    if let Some(fixed_esm_normal) = get_cached_normal(
                        info.cell_dx as i8,
                        info.cell_dy as i8,
                        info.local_vx,
                        info.local_vy,
                    ) {
                        esm_normal_vec3 = transform_mw_normal_to_bevy(Vec3::new(
                            fixed_esm_normal.x as f32,
                            fixed_esm_normal.y as f32,
                            fixed_esm_normal.z as f32,
                        ));
                    }
                }
            } else {
                // Interior vertex: No boundary crossing, use direct cell data
                if let Some(fixed_esm_normal) = get_cached_normal(0, 0, x_idx_mesh, y_idx_mesh) {
                    esm_normal_vec3 = transform_mw_normal_to_bevy(Vec3::new(
                        fixed_esm_normal.x as f32,
                        fixed_esm_normal.y as f32,
                        fixed_esm_normal.z as f32,
                    ));
                }
            }
            final_normals_bevy[vertex_index_flat] = esm_normal_vec3.normalize_or_zero().to_array();

            if is_boundary {
                let neighbors = &NEIGHBOR_LUT[vertex_index_flat];

                // Choose the neighbor that corresponds to the boundary we are on.
                // Since color fixing is a direct lookup, we grab the first valid neighbor
                // that crosses into the adjacent cell.
                let info = if x_idx_mesh == 0 {
                    &neighbors[2]
                }
                // Left
                else if x_idx_mesh == lx_max_idx {
                    &neighbors[3]
                }
                // Right
                else if y_idx_mesh == 0 {
                    &neighbors[0]
                }
                // Up
                else {
                    &neighbors[1]
                }; // Down

                if let Some(fixed_esm_color) = get_cached_color(
                    info.cell_dx as i8,
                    info.cell_dy as i8,
                    info.local_vx,
                    info.local_vy,
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

    let heightfield_collider = create_heightfield_collider(&positions_bevy);
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions_bevy);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, final_normals_bevy);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs_bevy);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, final_vclr_bevy);
    mesh.insert_indices(Indices::U32(indices_list));
    if let Err(e) = mesh.generate_tangents() {
        warn!("error generating tangents while creating landscape: {}", e);
    }

    Some((mesh, heightfield_collider))
}
fn setup_terrain_2d_array(texture_images: Vec<Image>) -> Image {
    let mut texture_array_image = setup_2d_image_array(texture_images.len() as u32, texture_images);

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
        let mut row = Vec::with_capacity(LAND_SIDE_VERTICES);
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
    let scale_xz = CELL_SIZE_METERS;
    let scale = Vec3::new(scale_xz, 1.0, scale_xz);

    Collider::heightfield(grid, scale)
}
fn setup_2d_image_array(mut len: u32, image_array: Vec<Image>) -> Image {
    //concat all images into 1 u8 vec
    let mut all_alpha_array_pixel_data = Vec::new();
    let mut image_width = 0;
    let mut image_height = 0;
    let mut image_format = TextureFormat::R8Unorm; //just a default, this is modified in the loop

    for mut image in image_array {
        let pixel_data = image.data.take().expect("Image had no data");
        // TODO:: find some way to not use this fix for wgsl shader not making this a 2d array
        // unless it has at least 2 textures
        // for now we just duplicate the image so it's always a 2d array
        if len < 2 {
            all_alpha_array_pixel_data.extend_from_slice(&pixel_data);
            len = 2;
        }
        all_alpha_array_pixel_data.extend_from_slice(&pixel_data);
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

fn cancel_entity_landscape_load(commands: &mut Commands, entity: Entity, err: &str) {
    commands.entity(entity).remove::<LandscapeAwaitingImages>();
    commands.entity(entity).remove::<LandscapeLoader>();
    warn!("landscape loading failed! {}", err);
}
#[inline]
fn transform_mw_normal_to_bevy(mw_normal: Vec3) -> Vec3 {
    Vec3::new(
        -mw_normal.x / 127.0,
        mw_normal.z / 127.0,
        mw_normal.y / 127.0,
    )
    .normalize_or_zero()
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

pub fn generate_alpha_maps_for_active_layers(
    landscapes: &HashMap<(i32, i32), Landscape>,
    current_cell_grid_x: i32,
    current_cell_grid_y: i32,
    image_handles_for_cell_len: usize,
    ltex_id_to_slot_index_map: HashMap<u16, usize>,
    image_scale_factor: u32,
) -> Result<Vec<Image>, String> {
    // Returning Vec<Image>
    if image_scale_factor == 0 {
        return Err("image_scale_factor cannot be zero.".to_string());
    }

    let num_layers = image_handles_for_cell_len;
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
            landscapes,
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

    // These define the inclusive end of the *texture coordinate indices* we are interested in
    // to generate (texture_quad_dimension + 1) sample points.
    // Corresponds to `minCoord + num_quads` from C++'s `maxRow = minRow + textureSize * size` logic.
    let max_row_tex_coord_for_range_input = min_row_tex_coord + texture_quad_dimension as i32;
    let max_col_tex_coord_for_range_input = min_col_tex_coord + texture_quad_dimension as i32;

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
    landscapes: &HashMap<(i32, i32), Landscape>,
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
    match landscapes.get(&(cell_x, cell_y)) {
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
    landscapes: &HashMap<(i32, i32), Landscape>,
    target_cell_x: i32,
    target_cell_y: i32,
) -> Result<HashSet<u16>, String> {
    let mut unique_ltex_ids = HashSet::new();

    // This closure will be called for every sample point.
    // It fetches the texture ID and adds it to our set.
    let sample_handler = |sample: CellSample| {
        let texture_id = get_texture_id_from_landscape(
            landscapes,
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
/// call load_detailed_data if the landscape has no detailed data yet
fn blocking_load_detailed_data_if_needed(current_cell_x: i32, current_cell_y: i32) {
    let lock = ALL_LANDSCAPES
        .get()
        .expect("ALL_LANDSCAPES not initialized");
    let needs_load = {
        let map = lock.read().unwrap();

        if let Some(landscape) = map.get(&(current_cell_x, current_cell_y)) {
            landscape.detailed_data.is_none()
        } else {
            return;
        }
    };
    if needs_load {
        let Ok(mut reader) = ESM_READER.get().unwrap().write() else {
            return;
        };
        let mut map = lock.write().unwrap();
        if let Some(landscape) = map.get_mut(&(current_cell_x, current_cell_y)) {
            if let Err(e) = landscape.load_detailed_data(&mut reader) {
                dbg!(e);
                return;
            }
        }
    }
}
