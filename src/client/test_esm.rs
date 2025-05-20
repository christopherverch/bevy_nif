use bevy::{
    asset::{AssetServer, Assets, Handle, RenderAssetUsages},
    color::Color,
    image::Image,
    log::{error, info, warn},
    math::{Quat, Vec3},
    pbr::{MeshMaterial3d, StandardMaterial},
    render::{
        mesh::{Indices, Mesh, Mesh3d, PrimitiveTopology, VertexAttributeValues},
        render_resource::{Extent3d, TextureDimension, TextureFormat},
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
    collections::HashMap,
    f32::consts::{FRAC_PI_2, PI},
    path::Path,
    time::Instant,
};

use crate::terrain_material::{
    ATTRIBUTE_TERRAIN_TEXTURE_INDICES, TerrainBlendMaterial, TerrainMaterialParams,
};

pub fn test(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let mut image_handles = Vec::new();
    let mut ltex_id_to_slot_index_map = HashMap::new();
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
                                        record_id_for_messages, // Pass &str for messages
                                        &record_type_for_map_key, // Pass owned String for map key usage if needed by handler
                                        commands.reborrow(),
                                        &mut meshes,
                                        &mut materials,
                                        &mut images,
                                        &asset_server,
                                        &mut image_handles,
                                        &mut ltex_id_to_slot_index_map,
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
    commands.insert_resource(ImageHandles(image_handles));
    commands.insert_resource(CellUniqueTextureSet {
        ltex_id_to_slot_index: ltex_id_to_slot_index_map,
    });
}

const LAND_SIDE_VERTICES: usize = 65;
const LAND_NUM_VERTS: usize = LAND_SIDE_VERTICES * LAND_SIDE_VERTICES; // 65*65 = 4225
const MORROWIND_CELL_SIZE: f32 = 8192.0;
const MORROWIND_HEIGHT_SCALE: f32 = 8.0;

pub fn construct_landscape_mesh(
    payload: &LandPayloadData, // Using your defined LandPayloadData
    cell_grid_x: i32,          // Morrowind grid X of the cell
    cell_grid_y: i32,          // Morrowind grid Y of the cell (often depth)
) -> Option<(
    Vec<[f32; 3]>,
    Vec<[f32; 3]>,
    Vec<[f32; 2]>,
    Vec<[f32; 4]>,
    Vec<u32>,
)> {
    let vhgt_data = payload.vertex_heights.as_ref()?; // Vertex heights are essential

    // 1. Calculate all vertex heights (these are Z values in Morrowind's coordinate system)
    let mut morrowind_vertex_z_values = Vec::with_capacity(LAND_NUM_VERTS);
    for y_idx in 0..LAND_SIDE_VERTICES {
        // The first element in each row of mHeightData is an offset relative
        // to mHeightOffset to get the actual height of the first vertex in that row.
        let mut current_row_z_unscaled = vhgt_data.height_offset
            + (vhgt_data.height_data[y_idx * LAND_SIDE_VERTICES + 0] as f32);
        morrowind_vertex_z_values.push(current_row_z_unscaled * 0.01);

        for x_idx in 1..LAND_SIDE_VERTICES {
            // Each successive byte in the row is then relative to the previous vertex in that row.
            current_row_z_unscaled +=
                vhgt_data.height_data[y_idx * LAND_SIDE_VERTICES + x_idx] as f32;
            morrowind_vertex_z_values.push(current_row_z_unscaled * 0.01);
        }
    }

    let mut positions_bevy: Vec<[f32; 3]> = Vec::with_capacity(LAND_NUM_VERTS);
    let mut normals_bevy: Vec<[f32; 3]> = Vec::with_capacity(LAND_NUM_VERTS);
    let mut uvs_bevy: Vec<[f32; 2]> = Vec::with_capacity(LAND_NUM_VERTS);
    let mut colors_bevy: Vec<[f32; 4]> = Vec::with_capacity(LAND_NUM_VERTS);

    // Cell origin in Morrowind world coordinates
    let cell_mw_origin_x = cell_grid_x as f32 * MORROWIND_CELL_SIZE;
    let cell_mw_origin_y = cell_grid_y as f32 * MORROWIND_CELL_SIZE; // Morrowind's Y (depth/north-south)

    for y_idx in 0..LAND_SIDE_VERTICES {
        // Iterates across Morrowind's Y dimension (depth)
        for x_idx in 0..LAND_SIDE_VERTICES {
            // Iterates across Morrowind's X dimension (width)
            let vertex_index_flat = y_idx * LAND_SIDE_VERTICES + x_idx;

            // Calculate vertex position in Morrowind's coordinate system
            let mw_x = cell_mw_origin_x + (x_idx as f32 / (LAND_SIDE_VERTICES - 1) as f32 * 40.0);
            let mw_y = cell_mw_origin_y + (y_idx as f32 / (LAND_SIDE_VERTICES - 1) as f32 * 40.0);
            let mw_z = morrowind_vertex_z_values[vertex_index_flat];

            // Convert Morrowind coordinates (X right, Y fwd, Z up)
            // to Bevy coordinates (X right, Y up, Z towards camera/south)
            positions_bevy.push([-mw_x, -mw_z, -mw_y]);

            // Handle Normals
            if let Some(normals_data) = payload.vertex_normals.as_ref() {
                let esm_normal = normals_data[vertex_index_flat];
                let n_mw_x = esm_normal.x as f32 / 127.0;
                let n_mw_y = esm_normal.y as f32 / 127.0; // Normal along Morrowind's Y (depth)
                let n_mw_z = esm_normal.z as f32 / 127.0; // Normal along Morrowind's Z (height)

                // Convert normal: Bevy_nX = mw_nX, Bevy_nY = mw_nZ, Bevy_nZ = -mw_nY
                // Ensure normalization after scaling and transformation
                let bevy_normal = Vec3::new(n_mw_x, n_mw_z, -n_mw_y).normalize_or_zero();
                normals_bevy.push(bevy_normal.into());
            } else {
                // Default normal (Y-up in Bevy) if none provided.
                // Alternatively, you might want to compute them from the geometry.
                normals_bevy.push([0.0, 1.0, 0.0]);
            }

            // Handle UVs (simple planar mapping for the whole cell)
            // y_idx corresponds to V in UV, but Morrowind's Y often goes "deeper" into the screen.
            // Standard UVs: (0,0) at one corner, (1,1) at the opposite.
            // If y_idx=0 is "top" of texture and y_idx=64 is "bottom".
            uvs_bevy.push([
                x_idx as f32 / (LAND_SIDE_VERTICES - 1) as f32, // U = 0.0 to 1.0
                y_idx as f32 / (LAND_SIDE_VERTICES - 1) as f32, // V = 0.0 to 1.0
                                                                // Some engines flip V: 1.0 - (y_idx / ...)
            ]);

            // Handle Colors
            if let Some(colors_data) = payload.vertex_colors.as_ref() {
                let esm_color = colors_data[vertex_index_flat];
                colors_bevy.push([
                    esm_color.r as f32 / 255.0,
                    esm_color.g as f32 / 255.0,
                    esm_color.b as f32 / 255.0,
                    1.0, // Alpha, assuming opaque
                ]);
            } else {
                colors_bevy.push([1.0, 1.0, 1.0, 1.0]); // Default to white
            }
        }
    }

    // Define triangle indices
    let mut indices = Vec::new();
    let quads_per_side = LAND_SIDE_VERTICES - 1;
    for y_quad in 0..quads_per_side {
        for x_quad in 0..quads_per_side {
            // Vertex indices of the current quad
            let v00 = (y_quad * LAND_SIDE_VERTICES + x_quad) as u32; // Top-left
            let v10 = (y_quad * LAND_SIDE_VERTICES + (x_quad + 1)) as u32; // Top-right
            let v01 = ((y_quad + 1) * LAND_SIDE_VERTICES + x_quad) as u32; // Bottom-left
            let v11 = ((y_quad + 1) * LAND_SIDE_VERTICES + (x_quad + 1)) as u32; // Bottom-right

            // Triangle 1 (ensure winding order is counter-clockwise for Bevy's default front face)
            // Assuming Y-up and looking down -Z, (v00, v01, v11) should be CCW
            indices.push(v00);
            indices.push(v01);
            indices.push(v11);

            // Triangle 2
            indices.push(v00);
            indices.push(v11);
            indices.push(v10);
        }
    }
    Some((positions_bevy, normals_bevy, uvs_bevy, colors_bevy, indices))

    // If normals were not provided by ESM and you used a default,
    // you might consider computing them after setting positions and indices,
    // though terrain often benefits from the explicit normals from the file.
    // if payload.vertex_normals.is_none() {
    //     mesh.compute_flat_normals(); // Or a custom smooth normal calculation
    // }
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
pub fn try_parse_and_print_record(
    reader: &mut EsmReader,
    rec_name_bytes: &[u8; 4],     // Pass the raw bytes for direct comparison
    record_id_for_messages: &str, // String version for error messages
    record_id_for_map_key: &str,  // String version for map keys (some load fns use it)
    mut commands: Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    images: &mut ResMut<Assets<Image>>,
    asset_server: &Res<AssetServer>,
    image_handles: &mut Vec<Handle<Image>>,
    ltex_id_to_slot_index_map: &mut HashMap<u16, usize>,
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
        setup_image_handles(tex, asset_server, image_handles, ltex_id_to_slot_index_map);
    }

    if *rec_name_bytes == *b"LAND" {
        let landscape = Landscape::load(reader).unwrap();
        if landscape.grid_x == 0 && landscape.grid_y == 0 {
            commands.insert_resource(LandscapeResource(landscape));
        }
    }
}

#[derive(Resource, Default)]
pub struct ImageHandles(Vec<Handle<Image>>);
#[derive(Resource, Default)]
pub struct TextureArrayHandle(Handle<Image>);
pub fn setup_image_handles(
    texture: LandTexture,
    asset_server: &Res<AssetServer>,
    image_handles: &mut Vec<Handle<Image>>,
    ltex_id_to_slot_index_map: &mut HashMap<u16, usize>,
) {
    if texture.index == 72
        || texture.index == 73
        || texture.index == 75
        || texture.index == 85
        || texture.index == 71
        || texture.index == 102
        || texture.index == 4
        || texture.index == 16
        || texture.index == 19
        || texture.index == 66
        || texture.index == 69
    {
        let texture_path = format!("textures/{}", texture.texture_file);
        println!("texture path: {}", texture_path);
        image_handles.push(asset_server.load(texture_path));
        ltex_id_to_slot_index_map.insert(texture.index as u16, image_handles.len() - 1);
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
    let texture_array_image = Image::new(
        Extent3d {
            width: image_width,
            height: image_height,
            depth_or_array_layers: 11, // Set this to your number of textures (e.g., 11)
        },
        TextureDimension::D2, // Use D2 for the dimension of each layer
        all_pixel_data,
        image_format,
        RenderAssetUsages::default(), // Or be more specific
    );
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
                construct_landscape_mesh(land, 0, 0).unwrap();
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
            let vtex_attr = generate_vertex_texture_indices_attribute(
                &land.texture_indices.unwrap(),
                &cell_texture_set,
            );
            //println!("vtex_attr: {:?}", vtex_attr);
            mesh.insert_attribute(ATTRIBUTE_TERRAIN_TEXTURE_INDICES, vtex_attr);
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
                params: TerrainMaterialParams {
                    tiling_factor: 16.0, // How many times should each texture tile across the cell? Adjust as needed.
                    num_textures_in_array: 11, // Example: The number of unique textures you put in the array
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
                    translation: Vec3::new(0.0, 2.0, 0.0),
                    scale: Vec3::splat(0.5),
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
) -> Vec<[i32; 4]> {
    let mut attribute_data: Vec<[i32; 4]> = Vec::with_capacity(LAND_NUM_VERTS);
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
            //                                         blended with (1.0 - (r+g+b)).
            let ltex_ids_for_vertex_slots: [Option<u16>; 4] = [
                get_ltex_id_for_vtex_quad(vtex_indices, sw_tq_x, sw_tq_y), // Corresponds to VCLR.r weight
                get_ltex_id_for_vtex_quad(vtex_indices, sw_tq_x + 1, sw_tq_y), // Corresponds to VCLR.g weight
                get_ltex_id_for_vtex_quad(vtex_indices, sw_tq_x, sw_tq_y + 1), // Corresponds to VCLR.b weight
                get_ltex_id_for_vtex_quad(vtex_indices, sw_tq_x + 1, sw_tq_y + 1), // Base/4th texture
            ];

            for i in 0..4 {
                if let Some(ltex_id) = ltex_ids_for_vertex_slots[i] {
                    if let Some(&slot_index) = cell_textures.ltex_id_to_slot_index.get(&ltex_id) {
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
        }
    }
    attribute_data
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
