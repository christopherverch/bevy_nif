use std::{collections::HashMap, fs::File, io::Write, write};

use bevy::{
    asset::{Handle, RenderAssetUsages},
    image::Image,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
use esm_parser_lib::LandPayloadData;
const LAND_TEXTURE_SIZE_CONST: usize = 16; // If LAND_NUM_TEXTURES is 256 (16*16)
const LAND_NUM_TEXTURES_CONST: usize = LAND_TEXTURE_SIZE_CONST * LAND_TEXTURE_SIZE_CONST;

pub fn generate_alpha_maps_for_active_layers(
    payload: &LandPayloadData,
    image_handles_for_cell: &Vec<Handle<Image>>, // These are the diffuse textures for the cell
    ltex_id_to_slot_index_map: &HashMap<u16, usize>, // Maps LTEX ID to slot in image_handles_for_cell
    image_scale_factor: u32,
) -> Result<Vec<Image>, String> {
    if image_scale_factor == 0 {
        return Err("image_scale_factor cannot be zero.".to_string());
    }

    let texture_indices_grid_slice = match payload.texture_indices.as_ref() {
        Some(indices) => indices.as_slice(),
        None => return Err("LandPayloadData does not contain texture_indices.".to_string()),
    };

    let num_active_layers = image_handles_for_cell.len();
    if num_active_layers <= 1 {
        // OpenMW produces no blend maps for 0 or 1 unique texture layer on the cell
        return Ok(Vec::new());
    }

    // base_vtex_grid_dim is the dimension of the VTEX grid (e.g., 16 for a 16x16 VTEX grid)
    let base_vtex_grid_dim: usize = LAND_TEXTURE_SIZE_CONST;
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
            let raw_ltex_id = texture_indices_grid_slice[vtex_grid_linear_idx] - 1;

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

    // Your PGM saving logic for debugging can be adapted here.
    // For example:

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

    Ok(output_alpha_map_images)
}
