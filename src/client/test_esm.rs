use bevy::{
    asset::{AssetServer, Assets, Handle, RenderAssetUsages},
    color::Color,
    image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    log::{error, info, warn},
    math::{Quat, Vec3},
    pbr::{MeshMaterial3d, StandardMaterial},
    render::{
        mesh::{Indices, Mesh, Mesh3d, PrimitiveTopology, VertexAttributeValues},
        render_resource::{Extent3d, FilterMode, TextureDimension, TextureFormat, TextureUsages},
        view::Visibility,
    },
    transform::components::Transform,
    utils::default,
};
use bevy_ecs::{
    name::Name,
    resource::Resource,
    system::{Commands, Local, Res, ResMut},
};
use bevy_rapier3d::prelude::Collider;
use esm_parser_lib::{
    EsmReader, Header, LAND_NUM_TEXTURES, LandPayloadData, LandTexture, Landscape, TextDecoder,
    esm_reader::Windows1252Decoder,
};
use std::{
    collections::{HashMap, HashSet},
    f32::consts::{FRAC_PI_2, PI},
    path::Path,
    time::Instant,
};

use crate::{
    terrain_material::{
        ATTRIBUTE_TERRAIN_TEXTURE_INDICES, TerrainBlendMaterial, TerrainMaterialParams,
    },
    test_esm2::generate_alpha_maps_for_active_layers,
};

pub fn test(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let mut image_handles = Vec::new();
    let mut land_textures = Vec::new();
    let mut ltex_id_to_slot_index_map = HashMap::new();
    let mut landscape = None;
    let file_path_str = "Morrowind.esm"; // <--- CHANGE THIS
    let file_path = Path::new(file_path_str);

    if !file_path.exists() {
        eprintln!(
            "Error: File not found at '{}'. Please update the path in main.rs.",
            file_path.display()
        );
        return;
    }

    let mut reader = EsmReader::new();

    println!("\nAttempting to open: {}", file_path.display());
    let start_time = Instant::now();

    match reader.open_raw_path(
        file_path,
        Some(Box::new(Windows1252Decoder::default()) as Box<dyn TextDecoder>),
    ) {
        Ok(_) => {
            let duration = start_time.elapsed();
            println!(
                "Successfully opened '{}' in {:?}",
                file_path.display(),
                duration
            );

            let header: &Header = reader.get_header();
            print_header_information(header);

            println!("\n--- Reading Records (first few of each interesting type) ---");
            let parsed_counts: HashMap<String, usize> = HashMap::new();
            let records_to_show_each = 1;

            let target_record_types = [
                // "GMST", "BSGN", "CONT", "FACT", "LOCK", "PROB", "GLOB", "SKIL", "CLAS", "RACE",
                //"SOUN", "REGN", "MGEF", "SPEL", "BOOK", "WEAP", "ARMO", "MISC", "LIGH", "CREA",
                //"INGR", "CLOT", "APPA", "ALCH", "LEVI", "STAT", "DOOR", "LEVC", "ENCH", "ACTI",
                //"BODY", "REPA", "PGRD", "CELL", "NPC_", "SNDG", "AMMO", "LVLN", "SCPT", "DIAL",
                //"INFO", "LTEX", "WAYP", "LAND", "KEYM",
                "LAND", "LTEX",
            ];
            'record_loop: while reader.has_more_recs() {
                let mut all_targets_done = true;
                for rec_type_str_check in target_record_types {
                    if *parsed_counts.get(rec_type_str_check).unwrap_or(&0) < records_to_show_each {
                        all_targets_done = false;
                        break;
                    }
                }
                if all_targets_done {
                    println!(
                        "All targeted record types shown {} times. Stopping further parsing.",
                        records_to_show_each
                    );
                    break 'record_loop;
                }

                match reader.get_rec_name() {
                    Ok(rec_name) => {
                        let record_name_bytes = rec_name.data;
                        let record_id_string_val = rec_name.to_string_lossy();
                        //println!("record name: {}", record_id_string_val);
                        let record_type_for_map_key = record_id_string_val.to_string();
                        let record_id_for_messages = record_id_string_val.as_str();

                        match reader.get_rec_header() {
                            Ok(_flags) => {
                                let current_parsed_count =
                                    *parsed_counts.get(&record_type_for_map_key).unwrap_or(&0);
                                let should_parse_this_type_in_detail = target_record_types
                                    .contains(&record_type_for_map_key.as_str())
                                    && current_parsed_count < records_to_show_each;

                                if should_parse_this_type_in_detail {
                                    // Pass the original rec_name (Name struct) to the dispatcher
                                    try_parse_and_print_record(
                                        &mut reader,
                                        &record_name_bytes, // Pass the raw bytes for matching
                                        commands.reborrow(),
                                        &mut land_textures,
                                        &mut landscape,
                                    )
                                } else {
                                    if let Err(e) = reader.skip_record() {
                                        eprintln!(
                                            "    Error skipping record data for {}: {}",
                                            record_id_for_messages, e
                                        );
                                        break 'record_loop;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "  Error reading record header for {}: {}",
                                    record_id_for_messages, e
                                );
                                break 'record_loop;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error getting next record name: {}", e);
                        break 'record_loop;
                    }
                }
            }
            if reader.has_more_recs() {
                println!("...and possibly more records.");
            } else {
                println!("End of file reached.");
            }

            reader.close();
            println!("\nFile closed.");
        }
        Err(e) => {
            let duration = start_time.elapsed();
            eprintln!("\nError after {:?}: {}", duration, e);
        }
    }
    landscape
        .as_mut()
        .unwrap()
        .load_detailed_data(&mut reader)
        .unwrap();
    let mut indices = landscape
        .clone()
        .unwrap()
        .detailed_data
        .unwrap()
        .texture_indices
        .unwrap();
    println!("indices: {:?}", indices);
    for index in indices.iter_mut() {
        *index -= 1;
    }
    println!("indices: {:?}", indices);
    let unique_texture_indices: HashSet<u16> = indices.iter().cloned().collect();
    setup_image_handles(
        land_textures.clone(),
        &asset_server,
        &mut image_handles,
        &mut ltex_id_to_slot_index_map,
        unique_texture_indices,
    );
    for texture in &land_textures {
        println!("vtex_attr: {:?}", texture);
    }
    println!("image handles len: {}", image_handles.len());
    commands.insert_resource(ImageHandles(image_handles));
    commands.insert_resource(LandTextureResource(land_textures));
    commands.insert_resource(CellUniqueTextureSet {
        ltex_id_to_slot_index: ltex_id_to_slot_index_map,
    });
    commands.insert_resource(LandscapeResource(landscape.unwrap()));
}

const LAND_SIDE_VERTICES: usize = 65;
const LAND_NUM_VERTS: usize = LAND_SIDE_VERTICES * LAND_SIDE_VERTICES; // 65*65 = 4225
const MORROWIND_CELL_SIZE: f32 = 40.0;
const MORROWIND_HEIGHT_SCALE: f32 = 0.04;

/// Calculates the splatting weights and texture indices for a single vertex
/// based on its position relative to the VTEX grid.
fn calculate_single_vertex_splat_attributes(
    vtex_u_frac: f32, // Vertex's fractional U within VTEX grid (0.0 to VTEX_GRID_DIM - 1.0)
    vtex_v_frac: f32, // Vertex's fractional V within VTEX grid (0.0 to VTEX_GRID_DIM - 1.0)
    vtex_grid_data: &[u16],
    vtex_grid_dim_u32: u32, // e.g., 16
    ltex_id_to_slot_index: &HashMap<u16, usize>,
    default_slot_index: usize,
) -> ([f32; 4], [i32; 4]) {
    let x0_vtex_idx = (vtex_u_frac.floor() as u32).clamp(0, vtex_grid_dim_u32 - 1);
    let y0_vtex_idx = (vtex_v_frac.floor() as u32).clamp(0, vtex_grid_dim_u32 - 1);
    let x1_vtex_idx = (x0_vtex_idx + 1).clamp(0, vtex_grid_dim_u32 - 1);
    let y1_vtex_idx = (y0_vtex_idx + 1).clamp(0, vtex_grid_dim_u32 - 1);

    let get_ltex_id_from_grid =
        |gx: u32, gy: u32| -> u16 { vtex_grid_data[(gy * vtex_grid_dim_u32 + gx) as usize] };

    let ltex_id_at_v00 = get_ltex_id_from_grid(x0_vtex_idx, y0_vtex_idx);
    let ltex_id_at_v10 = get_ltex_id_from_grid(x1_vtex_idx, y0_vtex_idx);
    let ltex_id_at_v01 = get_ltex_id_from_grid(x0_vtex_idx, y1_vtex_idx);
    let ltex_id_at_v11 = get_ltex_id_from_grid(x1_vtex_idx, y1_vtex_idx);

    let mut influences_map: HashMap<u16, f32> = HashMap::new();
    let surrounding_ltex_ids_in_vtex_neighborhood = [
        ltex_id_at_v00,
        ltex_id_at_v10,
        ltex_id_at_v01,
        ltex_id_at_v11,
    ];

    let mut unique_ltex_ids_to_process: Vec<u16> = Vec::new();
    for &id in surrounding_ltex_ids_in_vtex_neighborhood.iter() {
        if id != 0 && !unique_ltex_ids_to_process.contains(&id) {
            unique_ltex_ids_to_process.push(id);
        }
    }

    if unique_ltex_ids_to_process.is_empty() {
        let mut weights = [0.0f32; 4];
        let mut indices = [-1i32; 4];
        if ltex_id_to_slot_index
            .values()
            .any(|&val| val == default_slot_index)
            || ltex_id_to_slot_index.is_empty()
        {
            // Check if default is usable
            weights[0] = 1.0;
            indices[0] = default_slot_index as i32;
        }
        return (weights, indices);
    }

    for &current_ltex_id_from_vtex_neighborhood in unique_ltex_ids_to_process.iter() {
        let p00 = if ltex_id_at_v00 == current_ltex_id_from_vtex_neighborhood {
            1.0
        } else {
            0.0
        };
        let p10 = if ltex_id_at_v10 == current_ltex_id_from_vtex_neighborhood {
            1.0
        } else {
            0.0
        };
        let p01 = if ltex_id_at_v01 == current_ltex_id_from_vtex_neighborhood {
            1.0
        } else {
            0.0
        };
        let p11 = if ltex_id_at_v11 == current_ltex_id_from_vtex_neighborhood {
            1.0
        } else {
            0.0
        };

        let u_sub_for_interp = vtex_u_frac - x0_vtex_idx as f32;
        let v_sub_for_interp = vtex_v_frac - y0_vtex_idx as f32;

        let interp_y0 = lerp(p00, p10, u_sub_for_interp);
        let interp_y1 = lerp(p01, p11, u_sub_for_interp);
        let influence = lerp(interp_y0, interp_y1, v_sub_for_interp);

        if influence > 0.0001 {
            influences_map.insert(current_ltex_id_from_vtex_neighborhood, influence);
        }
    }

    let mut sorted_influences: Vec<(u16, f32)> = influences_map.into_iter().collect();
    sorted_influences.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut vertex_weights = [0.0f32; 4];
    let mut vertex_slot_indices = [-1i32; 4];
    let mut total_influence_of_top_4 = 0.0;
    let num_to_take = sorted_influences.len().min(4);

    for i in 0..num_to_take {
        let ltex_id = sorted_influences[i].0;
        let influence = sorted_influences[i].1;

        if let Some(&slot_idx) = ltex_id_to_slot_index.get(&ltex_id) {
            vertex_slot_indices[i] = slot_idx as i32;
            vertex_weights[i] = influence;
            total_influence_of_top_4 += influence;
        }
    }

    if total_influence_of_top_4 > 0.001 {
        for i in 0..num_to_take {
            if vertex_slot_indices[i] != -1 {
                vertex_weights[i] /= total_influence_of_top_4;
            }
        }
    } else {
        // Fallback if total influence is ~0 or no valid mapped textures were found
        let mut assigned_default = false;
        if ltex_id_to_slot_index
            .values()
            .any(|&val| val == default_slot_index)
            || ltex_id_to_slot_index.is_empty()
        {
            vertex_slot_indices[0] = default_slot_index as i32;
            vertex_weights[0] = 1.0;
            assigned_default = true;
        }
        if assigned_default {
            // Clear other slots if default was assigned
            for i in 1..4 {
                vertex_slot_indices[i] = -1;
                vertex_weights[i] = 0.0;
            }
        } else { // Could not even assign default (e.g. map is empty and default_slot_index isn't 0 for an implicit texture)
            // Leave as all zeros / -1s
        }
    }
    (vertex_weights, vertex_slot_indices)
}
const VTEX_GRID_DIM: u32 = 16;
pub fn construct_landscape_mesh(
    payload: &LandPayloadData, // Assuming LandPayloadData is your struct from loadland.rs
    cell_grid_x: i32,
    cell_grid_y: i32,
    // Parameters for splatting (kept as per your function signature)
    ltex_id_to_slot_index: &HashMap<u16, usize>,
    default_slot_index: usize,
) -> Option<(
    Vec<[f32; 3]>, // positions_bevy
    Vec<[f32; 3]>, // normals_bevy
    Vec<[f32; 2]>, // uvs_bevy
    Vec<[f32; 4]>, // original_vclr_bevy (original vertex colors for tinting)
    Vec<u32>,      // indices
)> {
    let vhgt_data = payload.vertex_heights.as_ref()?; // From LAND.VHGT
    let vtex_grid_slice = payload.texture_indices.as_ref()?.as_slice(); // From LAND.VTEX

    // --- 1. Corrected Heightmap Generation ---
    // This now matches the C++ accumulation logic
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
                // Your original: Vec3::new(n_mw_x, n_mw_z, n_mw_y).normalize_or_zero();
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
            uvs_bevy.push([norm_u_mesh, 1.0 - norm_v_mesh]); // Flipped V to match OpenMW C++
        }
    }

    // --- 6. Index Buffer Generation (Diamond Pattern) ---
    // This matches the core triangulation pattern from OpenMW C++
    // (without LOD stitching).
    let mut indices_list = Vec::new();
    let quads_per_side = LAND_SIDE_VERTICES - 1;
    let n_verts_per_side_u32 = LAND_SIDE_VERTICES as u32;

    for qy_idx in 0..quads_per_side {
        for qx_idx in 0..quads_per_side {
            let v_a = qy_idx as u32 * n_verts_per_side_u32 + qx_idx as u32;
            let v_b = qy_idx as u32 * n_verts_per_side_u32 + (qx_idx + 1) as u32;
            let v_c = ((qy_idx + 1) as u32 * n_verts_per_side_u32 + qx_idx as u32);
            let v_d = ((qy_idx + 1) as u32 * n_verts_per_side_u32 + (qx_idx + 1) as u32);

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
pub fn print_header_information(header: &Header) {
    println!("\n--- Header Information ---");
    println!("  File Version (float): {}", header.esm_version_f());
    println!("  Author: '{}'", header.get_author());
    let desc_preview = header.get_desc().chars().take(70).collect::<String>();
    println!(
        "  Description: '{}'{}",
        desc_preview,
        if header.get_desc().len() > 70 {
            "..."
        } else {
            ""
        }
    );
    println!("  Number of Records: {}", header.get_record_count());

    if !header.get_game_files().is_empty() {
        println!("  Master Files:");
        for master in header.get_game_files() {
            println!("    - Name: {}, Size: {}", master.name, master.size);
        }
    }
}

#[derive(Resource, Default)]
pub struct LandscapeResource(Landscape);
#[derive(Resource, Default)]
pub struct LandTextureResource(Vec<LandTexture>);
pub fn try_parse_and_print_record(
    reader: &mut EsmReader,
    rec_name_bytes: &[u8; 4], // Pass the raw bytes for direct comparison
    mut commands: Commands,
    textures: &mut Vec<LandTexture>,
    landscape_in: &mut Option<Landscape>,
) {
    // Simple dispatch based on first letter for now, or could be more sophisticated
    // For now, just try part1, and if it doesn't handle it, try part2.
    fn print_record_details<F>(record_tag_str: &str, print_logic: F)
    where
        F: FnOnce(),
    {
        print!("  [{}] ", record_tag_str);
        print_logic();
    }
    if *rec_name_bytes == *b"LTEX" {
        let tex = LandTexture::load(reader).unwrap().0;
        println!("tex: {:?}", tex);
        textures.push(tex);
    }

    if *rec_name_bytes == *b"LAND" {
        let landscape = Landscape::load(reader).unwrap();
        if landscape.grid_x == -2 && landscape.grid_y == -9 {
            println!("found land");
            *landscape_in = Some(landscape);
        }
    }
}

#[derive(Resource, Default)]
pub struct ImageHandles(Vec<Handle<Image>>);
#[derive(Resource, Default)]
pub struct TextureArrayHandle(Handle<Image>);
pub fn setup_image_handles(
    textures: Vec<LandTexture>,
    asset_server: &Res<AssetServer>,
    image_handles: &mut Vec<Handle<Image>>,
    ltex_id_to_slot_index_map: &mut HashMap<u16, usize>,
    indices: HashSet<u16>,
) {
    for texture in textures {
        if indices.contains(&(texture.index as u16)) {
            let texture_path = format!("textures/{}", texture.texture_file);
            println!("texture path: {} {}", texture_path, texture.index);
            image_handles.push(asset_server.load(texture_path));
            ltex_id_to_slot_index_map.insert(texture.index as u16, image_handles.len() - 1);
        }
    }
}

pub fn await_image_loading(
    image_handles: Res<ImageHandles>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
    mut landscape: ResMut<LandscapeResource>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    mut do_once: Local<bool>,
    mut custom_materials: ResMut<Assets<TerrainBlendMaterial>>,
    cell_texture_set: Res<CellUniqueTextureSet>,
) {
    if *do_once {
        return;
    }
    for image_handle in &image_handles.0 {
        if images.get(image_handle).is_none() {
            return;
        }
    }
    //we have all the images
    let mut all_pixel_data = Vec::new();
    let mut image_width = 0;
    let mut image_height = 0;
    let mut image_format = TextureFormat::Rgba8UnormSrgb; // Assuming a default/first
    for (_i, handle) in image_handles.0.iter().enumerate() {
        println!("path: {:?}", handle.path());
        if let Some(image) = images.get_mut(handle) {
            image_width = image.width();
            image_height = image.height();
            image_format = image.texture_descriptor.format;
            all_pixel_data.extend_from_slice(&image.data.clone().unwrap());
        } else {
            // Handle error: image not loaded (should be pre-checked)
            return; // From a function trying to create the array
        }
    }
    let mut texture_array_image = Image::new(
        Extent3d {
            width: image_width,
            height: image_height,
            depth_or_array_layers: image_handles.0.len() as u32, // Set this to your number of textures (e.g., 11)
        },
        TextureDimension::D2, // Use D2 for the dimension of each layer
        all_pixel_data,
        image_format,
        RenderAssetUsages::RENDER_WORLD, // Or be more specific
    );
    texture_array_image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat, // Or ClampToEdge for W if not used for 3D sampling
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear, // Or nearest if you don't have mipmaps / want that style
        anisotropy_clamp: 4,
        ..Default::default()
    });
    setup_landscape(
        texture_array_image,
        landscape,
        commands,
        meshes,
        materials,
        images,
        asset_server,
        custom_materials,
        cell_texture_set,
        image_handles,
    );
    *do_once = true;
}

fn setup_landscape(
    texture_image: Image,
    mut landscape: ResMut<LandscapeResource>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    mut custom_materials: ResMut<Assets<TerrainBlendMaterial>>,
    cell_texture_set: Res<CellUniqueTextureSet>,
    image_handles: Res<ImageHandles>,
) {
    let file_path_str = "Morrowind.esm"; // <--- CHANGE THIS
    let file_path = Path::new(file_path_str);

    if !file_path.exists() {
        eprintln!(
            "Error: File not found at '{}'. Please update the path in main.rs.",
            file_path.display()
        );
        return;
    }

    let mut reader = EsmReader::new();

    match reader.open_raw_path(
        file_path,
        Some(Box::new(Windows1252Decoder::default()) as Box<dyn TextDecoder>),
    ) {
        Ok(_) => {
            landscape.0.load_detailed_data(&mut reader).unwrap();
            let land = landscape.0.detailed_data.as_deref().unwrap();
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            );
            let (positions_bevy, normals_bevy, uvs_bevy, colors_bevy, indices) =
                construct_landscape_mesh(land, 0, 0, &cell_texture_set.ltex_id_to_slot_index, 0)
                    .unwrap();
            let positions_vec3: Vec<Vec3> = positions_bevy
                .iter()
                .map(|pos_array| Vec3::from_array(*pos_array))
                .collect();
            let indices_vec: Vec<[u32; 3]> = indices
                .chunks_exact(3) // Creates an iterator over slices of 3 elements (&[u32])
                .map(|chunk| {
                    // Convert the slice chunk [idx0, idx1, idx2] into an array [u32; 3]
                    // This assumes chunk will always have exactly 3 elements due to chunks_exact
                    [chunk[0], chunk[1], chunk[2]]
                })
                .collect();
            let (vtex_attr, vtex_contributions) = generate_vertex_texture_indices_attribute(
                &land.texture_indices.unwrap(),
                &cell_texture_set,
            );
            //println!("vtex_attr: {:?}", vtex_attr);
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions_bevy);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals_bevy);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs_bevy);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors_bevy);
            mesh.insert_indices(Indices::U32(indices));
            if let Err(e) = mesh.generate_tangents() {
                println!("error generating tangents: {}", e);
            } else {
                println!("ok!");
            }

            let alpha_map_array = generate_alpha_maps_for_active_layers(
                land,
                &image_handles.0,
                &cell_texture_set.ltex_id_to_slot_index,
                2,
            )
            .unwrap();
            let mut all_pixel_data = Vec::new();
            let mut image_width = 0;
            let mut image_height = 0;
            let mut image_format = TextureFormat::Rgba8UnormSrgb; // Assuming a default/first

            println!("alpha map array len: {}", alpha_map_array.len());
            for image in alpha_map_array {
                all_pixel_data.extend_from_slice(&image.data.clone().unwrap());
                image_width = image.width();
                image_height = image.height();
                image_format = image.texture_descriptor.format;
            }
            let mut alpha_texture_array_image = Image::new(
                Extent3d {
                    width: image_width,
                    height: image_height,
                    depth_or_array_layers: image_handles.0.len() as u32, // Set this to your number of textures (e.g., 11)
                },
                TextureDimension::D2, // Use D2 for the dimension of each layer
                all_pixel_data,
                image_format,
                RenderAssetUsages::RENDER_WORLD, // Or be more specific
            );
            alpha_texture_array_image.texture_descriptor.usage =
                TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
            let alpha_texture_array_h = images.add(alpha_texture_array_image);

            if mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_none()
                || mesh.attribute(ATTRIBUTE_TERRAIN_TEXTURE_INDICES).is_none()
                || mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_none()
                || mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_none()
                || mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_none()
                || mesh.indices().is_none()
                || mesh.attribute(Mesh::ATTRIBUTE_TANGENT).is_none()
            {
                println!("failed");
            }
            // println!("{:?}", mesh);
            let texture_handle: Handle<Image> = images.add(texture_image);
            //let texture_handle: Handle<Image> = asset_server.load("textures/Tx_ashlands_06.tga");
            /*           let terrain_material_handle = materials.add(StandardMaterial {
                base_color_texture: Some(texture_handle.clone()),
                // Vertex colors (if present on the mesh) will modulate this texture.
                // If you want vertex colors to be more prominent, set base_color to Color::WHITE.
                base_color: Color::WHITE, // Good default when using a texture
                perceptual_roughness: 0.8, // Adjust as needed
                metallic: 0.1,            // Terrain is usually not very metallic
                ..default()
            });*/
            let mesh_h = meshes.add(mesh);
            let rotation = Quat::from_rotation_z(0.0);
            let material_handle = custom_materials.add(TerrainBlendMaterial {
                source_textures: texture_handle,
                alpha_map_array: alpha_texture_array_h,
                params: TerrainMaterialParams {
                    num_textures_in_array: image_handles.0.len() as u32, // Example: The number of unique textures you put in the array
                    diffuse_tiling_factor: 16.0,
                },
            });
            commands.spawn((
                Visibility::Visible,
                Name::new("land"),
                Mesh3d(mesh_h),
                MeshMaterial3d(material_handle),
                Collider::trimesh(positions_vec3, indices_vec).unwrap(),
                Transform {
                    rotation,
                    translation: Vec3::new(2.0, 2.0, -2.0),
                    scale: Vec3::splat(1.0),
                },
            ));
        }
        Err(_e) => {
            println!("failed to open file");
        }
    }
}
#[derive(Resource, Default)]
pub struct CellUniqueTextureSet {
    pub ltex_id_to_slot_index: HashMap<u16, usize>,
}
fn generate_vertex_texture_indices_attribute(
    vtex_indices: &[u16; LAND_NUM_TEXTURES], // From LandPayloadData.texture_indices
    cell_textures: &CellUniqueTextureSet,    // Contains mapping from LTEX ID to unique slot index
) -> (Vec<[i32; 4]>, Vec<[f32; 4]>) {
    let mut attribute_data: Vec<[i32; 4]> = Vec::with_capacity(LAND_NUM_VERTS);
    let mut contribution_data: Vec<[f32; 4]> = Vec::with_capacity(LAND_NUM_VERTS);
    const SENTINEL_INDEX: i32 = -1; // For unused texture slots if a vertex is affected by < 4 textures

    for vy_vert in 0..LAND_SIDE_VERTICES {
        // 0 to 64 (inclusive, if LAND_SIDE_VERTICES is 65)
        for vx_vert in 0..LAND_SIDE_VERTICES {
            // 0 to 64 (inclusive)
            let mut tex_slots_for_this_vertex = [SENTINEL_INDEX; 4];

            // Determine the VTEX quad coordinates for the four quads surrounding this vertex.
            // The vertex (vx_vert, vy_vert) is the top-right corner of the vertex-cell
            // ((vx_vert-1), (vy_vert-1)).
            // A common interpretation relates VCLR.rgb to textures from specific relative quads:
            // sw_tq_x, sw_tq_y refers to the texture quad that this vertex is the NE corner of.
            let sw_tq_x = (vx_vert as i32 - 1) / 4;
            let sw_tq_y = (vy_vert as i32 - 1) / 4;

            // The order of these 4 LTEX IDs needs to be consistent with how the shader
            // will use the R, G, B channels of VCLR and a calculated fourth weight.
            // A typical Morrowind-like setup (this order NEEDS VERIFICATION for exactitude):
            // Slot 0 (tex_slots_for_this_vertex[0]): Texture for VCLR.r weight (e.g., SW quad: (sw_tq_x, sw_tq_y))
            // Slot 1 (tex_slots_for_this_vertex[1]): Texture for VCLR.g weight (e.g., SE quad: (sw_tq_x + 1, sw_tq_y))
            // Slot 2 (tex_slots_for_this_vertex[2]): Texture for VCLR.b weight (e.g., NW quad: (sw_tq_x, sw_tq_y + 1))
            // Slot 3 (tex_slots_for_this_vertex[3]): Base/fourth texture (e.g., NE quad: (sw_tq_x + 1, sw_tq_y + 1)),
            //                                         blended with (1.0 - (r+g+b))
            let ltex_ids_for_vertex_slots: [Option<u16>; 4] = [
                get_ltex_id_for_vtex_quad(vtex_indices, sw_tq_x, sw_tq_y), // Corresponds to VCLR.r weight
                get_ltex_id_for_vtex_quad(vtex_indices, sw_tq_x + 1, sw_tq_y), // Corresponds to VCLR.g weight
                get_ltex_id_for_vtex_quad(vtex_indices, sw_tq_x, sw_tq_y + 1), // Corresponds to VCLR.b weight
                get_ltex_id_for_vtex_quad(vtex_indices, sw_tq_x + 1, sw_tq_y + 1), // Base/4th texture
            ];
            let contribution_amount = compute_rgba(ltex_ids_for_vertex_slots);

            for i in 0..4 {
                if let Some(ltex_id) = ltex_ids_for_vertex_slots[i] {
                    if let Some(&slot_index) =
                        cell_textures.ltex_id_to_slot_index.get(&(ltex_id - 1))
                    {
                        tex_slots_for_this_vertex[i] = slot_index as i32;
                    } else {
                        // LTEX ID from VTEX was valid but not in this cell's unique texture set.
                        // This might happen if prepare_cell_textures filtered it (e.g. due to
                        // missing file) or if ltex_id_to_slot_index is incomplete.
                        // It will remain SENTINEL_INDEX. The shader must handle this.
                        warn!(
                            "LTEX ID {} (from VTEX) not mapped to a unique slot for cell. Slot {} for vertex ({},{}) will be sentinel.",
                            ltex_id, i, vx_vert, vy_vert
                        );
                    }
                }
            }
            attribute_data.push(tex_slots_for_this_vertex);
            contribution_data.push(contribution_amount);
        }
    }
    (attribute_data, contribution_data)
}
const VTEX_GRID_SIDE: usize = 16;
fn get_ltex_id_for_vtex_quad(
    vtex_indices: &[u16; LAND_NUM_TEXTURES],
    tx: i32,
    ty: i32,
) -> Option<u16> {
    if tx >= 0 && tx < VTEX_GRID_SIDE as i32 && ty >= 0 && ty < VTEX_GRID_SIDE as i32 {
        let index = (ty as usize) * VTEX_GRID_SIDE + (tx as usize);
        let ltex_id = vtex_indices[index];
        // Handle sentinel values if 0 or u16::MAX mean "no texture"
        if ltex_id == 0 || ltex_id == u16::MAX {
            return None;
        }
        Some(ltex_id)
    } else {
        None // Out of bounds
    }
}
fn compute_rgba<T>(options: [Option<T>; 4]) -> [f32; 4] {
    let count_some = options.iter().filter(|o| o.is_some()).count();

    let value_per_some = if count_some > 0 {
        1.0 / count_some as f32
    } else {
        0.0
    };

    options.map(|o| if o.is_some() { value_per_some } else { 0.0 })
}
pub struct SplatMapData {
    pub weights: Vec<[f32; 4]>, // RGBA weights for each splat map pixel
    pub indices: Vec<[i32; 4]>, // XYZW texture array slot indices for each splat map pixel
    pub width: u32,
    pub height: u32,
}
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}
pub fn generate_splatmap_for_chunk(
    vtex_data: &Vec<u16>,
    land_texture_grid_dim: u32,
    ltex_id_to_slot_index: &HashMap<u16, usize>,
    splatmap_width: u32,
    splatmap_height: u32,
    default_slot_index: usize,
) -> Result<SplatMapData, String> {
    if vtex_data.len() != (land_texture_grid_dim * land_texture_grid_dim) as usize {
        return Err(format!(
            "VTEX data length ({}) does not match dimensions ({}x{})",
            vtex_data.len(),
            land_texture_grid_dim,
            land_texture_grid_dim
        ));
    }
    if splatmap_width == 0 || splatmap_height == 0 {
        return Err("Splatmap dimensions must be greater than 0".to_string());
    }

    let mut splatmap_weights_data: Vec<[f32; 4]> =
        vec![[0.0; 4]; (splatmap_width * splatmap_height) as usize];
    let mut splatmap_indices_data: Vec<[i32; 4]> =
        vec![[-1; 4]; (splatmap_width * splatmap_height) as usize];

    let grid_dim_f32 = land_texture_grid_dim as f32;

    for sy in 0..splatmap_height {
        for sx in 0..splatmap_width {
            // 1. Map splat map pixel to fractional VTEX grid coordinates
            // Ensure we don't divide by zero if splatmap dimension is 1
            let norm_u = if splatmap_width > 1 {
                sx as f32 / (splatmap_width - 1) as f32
            } else {
                0.5
            };
            let norm_v = if splatmap_height > 1 {
                sy as f32 / (splatmap_height - 1) as f32
            } else {
                0.5
            };

            let vtex_u_frac = norm_u * (grid_dim_f32 - 1.0);
            let vtex_v_frac = norm_v * (grid_dim_f32 - 1.0);

            // 2. Get surrounding VTEX grid points
            let x0 = (vtex_u_frac.floor() as u32).clamp(0, land_texture_grid_dim - 1);
            let y0 = (vtex_v_frac.floor() as u32).clamp(0, land_texture_grid_dim - 1);
            let x1 = (x0 + 1).clamp(0, land_texture_grid_dim - 1);
            let y1 = (y0 + 1).clamp(0, land_texture_grid_dim - 1);

            // 3. Get LTEX IDs at these 4 VTEX points
            let get_ltex_id =
                |gx: u32, gy: u32| -> u16 { vtex_data[(gy * land_texture_grid_dim + gx) as usize] };

            let ltex_id_00 = get_ltex_id(x0, y0);
            let ltex_id_10 = get_ltex_id(x1, y0);
            let ltex_id_01 = get_ltex_id(x0, y1);
            let ltex_id_11 = get_ltex_id(x1, y1);

            // 4. Calculate "filtered influence" for each unique LTEX ID in the 2x2 neighborhood
            let mut influences: HashMap<u16, f32> = HashMap::new();
            let surrounding_ltex_ids = [ltex_id_00, ltex_id_10, ltex_id_01, ltex_id_11];

            for &current_ltex_id in surrounding_ltex_ids.iter() {
                if current_ltex_id == 0 {
                    continue;
                } // Skip if it's a "null" or default untextured LTEX ID from ESM
                influences.entry(current_ltex_id).or_insert_with(|| {
                    let val_00 = if ltex_id_00 == current_ltex_id {
                        1.0
                    } else {
                        0.0
                    };
                    let val_10 = if ltex_id_10 == current_ltex_id {
                        1.0
                    } else {
                        0.0
                    };
                    let val_01 = if ltex_id_01 == current_ltex_id {
                        1.0
                    } else {
                        0.0
                    };
                    let val_11 = if ltex_id_11 == current_ltex_id {
                        1.0
                    } else {
                        0.0
                    };

                    let u_sub = vtex_u_frac - x0 as f32; // Fractional part for u
                    let v_sub = vtex_v_frac - y0 as f32; // Fractional part for v

                    let interp_y0 = lerp(val_00, val_10, u_sub);
                    let interp_y1 = lerp(val_01, val_11, u_sub);
                    lerp(interp_y0, interp_y1, v_sub)
                });
            }

            // 5. Select Top 4 Influences, Convert to Slot Indices, and Normalize Weights
            let mut sorted_influences: Vec<(u16, f32)> = influences.into_iter().collect();
            sorted_influences
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let mut final_weights = [0.0f32; 4];
            let mut final_slot_indices = [-1i32; 4];
            let mut total_influence_of_top_4 = 0.0;

            for i in 0..4 {
                if i < sorted_influences.len() && sorted_influences[i].1 > 0.001 {
                    // Epsilon
                    let ltex_id = sorted_influences[i].0;
                    let influence = sorted_influences[i].1;

                    if let Some(&slot_index) = ltex_id_to_slot_index.get(&ltex_id) {
                        final_slot_indices[i] = slot_index as i32;
                        final_weights[i] = influence; // Store raw influence for now
                        total_influence_of_top_4 += influence;
                    } else {
                        // LTEX ID not found in map, could assign default or skip
                        // For simplicity, we'll let it be -1 and weight 0 if not caught later
                    }
                }
            }

            if total_influence_of_top_4 > 0.001 {
                // Epsilon
                for i in 0..4 {
                    final_weights[i] /= total_influence_of_top_4;
                }
            } else {
                // No significant textures found, or only unmapped ones.
                // Assign a default texture to the first slot.
                final_slot_indices[0] = default_slot_index as i32;
                final_weights[0] = 1.0;
                for i in 1..4 {
                    final_slot_indices[i] = -1;
                    final_weights[i] = 0.0;
                }
            }

            let current_pixel_index = (sy * splatmap_width + sx) as usize;
            splatmap_weights_data[current_pixel_index] = final_weights;
            splatmap_indices_data[current_pixel_index] = final_slot_indices;
        }
    }

    Ok(SplatMapData {
        weights: splatmap_weights_data,
        indices: splatmap_indices_data,
        width: splatmap_width,
        height: splatmap_height,
    })
}
pub fn test_splatmap() {}
