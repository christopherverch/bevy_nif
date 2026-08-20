use std::fmt::Write;
use std::path::Path;
use std::{env, fs};

const LAND_SIDE_VERTICES: usize = 65;
const MW_UNITS_PER_METER: f32 = 69.991251091;
const CELL_SIZE_METERS: f32 = 8192.0 / MW_UNITS_PER_METER;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("landscape_lut.rs");
    let temp_path = Path::new(&out_dir).join("landscape_lut.rs.tmp");
    let mut buffer = String::new();

    // 1. Write the struct definition so main.rs can use it
    writeln!(buffer, "#[derive(Copy, Clone)]").unwrap();
    writeln!(buffer, "pub struct NeighborInfo {{ pub cell_dx: i32, pub cell_dy: i32, pub local_vx: usize, pub local_vy: usize }}").unwrap();

    // 2. Write the NEIGHBOR_LUT
    writeln!(
        buffer,
        "pub const NEIGHBOR_LUT: [[NeighborInfo; 4]; {}] = [",
        LAND_SIDE_VERTICES * LAND_SIDE_VERTICES
    )
    .unwrap();

    for y in 0..LAND_SIDE_VERTICES {
        for x in 0..LAND_SIDE_VERTICES {
            // Define the 4 directions
            let neighbors = [
                (x as i32, y as i32 - 1), // Up
                (x as i32, y as i32 + 1), // Down
                (x as i32 - 1, y as i32), // Left
                (x as i32 + 1, y as i32), // Right
            ];

            write!(buffer, "[").unwrap();
            for (nx, ny) in neighbors {
                // Perform the resolution logic here during build time
                let side = LAND_SIDE_VERTICES as i32;
                let cell_dx = nx.div_euclid(side);
                let cell_dy = ny.div_euclid(side);
                let lx = nx.rem_euclid(side) as usize;
                let ly = ny.rem_euclid(side) as usize;

                write!(
                    buffer,
                    "NeighborInfo {{ cell_dx: {}, cell_dy: {}, local_vx: {}, local_vy: {} }},",
                    cell_dx, cell_dy, lx, ly
                )
                .unwrap();
            }
            writeln!(buffer, "],").unwrap();
        }
    }
    writeln!(buffer, "];").unwrap();

    let half_cell = CELL_SIZE_METERS / 2.0;
    let max_idx = (LAND_SIDE_VERTICES - 1) as f32;

    // Positions LUT
    writeln!(
        buffer,
        "pub const RELATIVE_POS_LUT: [f32; {}] = [",
        LAND_SIDE_VERTICES
    )
    .unwrap();
    for i in 0..LAND_SIDE_VERTICES {
        let val = (i as f32 / max_idx * CELL_SIZE_METERS) - half_cell;
        writeln!(buffer, "{:.20}_f32,", val).unwrap();
    }
    writeln!(buffer, "];").unwrap();

    // UV LUT
    writeln!(
        buffer,
        "pub const UV_LUT: [f32; {}] = [",
        LAND_SIDE_VERTICES
    )
    .unwrap();
    for i in 0..LAND_SIDE_VERTICES {
        let val = i as f32 / max_idx;
        writeln!(buffer, "{:.20}_f32,", val).unwrap();
    }
    writeln!(buffer, "];").unwrap();
    let needs_update = if let Ok(existing) = fs::read_to_string(&dest_path) {
        existing != buffer
    } else {
        true // File doesn't exist yet
    };

    if needs_update {
        // Write to a temporary file first
        fs::write(&temp_path, buffer).expect("Failed to write temp LUT file");
        // Atomic rename updates the timestamp ONLY if the content changed
        fs::rename(&temp_path, &dest_path).expect("Failed to rename LUT file");
    }
}
