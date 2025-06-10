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
    impl<T> MockHandle<T> {
        fn new_dummy(id: u32) -> Handle<Image> {
            // Assuming Handle<Image> is the target type
            // For testing, we often use weak handles if we don't have a full App/AssetServer
            Handle::weak_from_u128(id as u128)
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
        let (_reader, all_landscapes, _land_textures) = run_initial_parse().unwrap();
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
