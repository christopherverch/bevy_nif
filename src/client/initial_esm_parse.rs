use bevy_ecs::system::Commands;
use cdda_bevy::client::defs::{AllLandscapes, EsmReaderResource, LandTextureDefinitions};
use esm_parser_lib::{
    EsmReader, Header, LandTexture, Landscape, RefId, TextDecoder, esm_reader::Windows1252Decoder,
};
use std::{collections::HashMap, path::Path, time::Instant};

pub fn populate_esm_resources(mut commands: Commands) {
    if let Ok((reader, landscapes, land_textures)) = run_initial_parse() {
        commands.insert_resource(EsmReaderResource(reader));
        commands.insert_resource(AllLandscapes(landscapes));
        commands.insert_resource(LandTextureDefinitions(land_textures));
    }
}
pub fn run_initial_parse() -> Result<
    (
        EsmReader<'static>,
        HashMap<(i32, i32), Landscape>,
        HashMap<u32, LandTexture>,
    ),
    String,
> {
    let mut land_textures = HashMap::new();
    let file_path_str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Morrowind.esm");
    let file_path = Path::new(file_path_str);
    let mut landscapes = HashMap::new();
    if !file_path.exists() {
        eprintln!(
            "Error: File not found at '{}'. Please update the path in main.rs.",
            file_path.display()
        );
        return Err(format!(
            "
            Error: File not found at '{}'. Please update the path in main.rs.",
            file_path.display(),
        ));
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
                                        &mut land_textures,
                                        &mut landscapes,
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

            //           reader.close();
            //         println!("\nFile closed.");
            // we don't close because we put the reader in a resource rather than opening and
            // closing the file each time
        }
        Err(e) => {
            let duration = start_time.elapsed();
            eprintln!("\nError after {:?}: {}", duration, e);
        }
    }
    // we have to insert this because it's always available as the "default texture"
    let default_texture = LandTexture {
        texture_file: "_land_default.tga".to_string(),
        // no idea what to put for id
        id: RefId::new(),
        index: u16::MAX as u32,
    };
    land_textures.insert(default_texture.index, default_texture);
    Ok((reader, landscapes, land_textures))
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

pub fn try_parse_and_print_record(
    reader: &mut EsmReader,
    rec_name_bytes: &[u8; 4], // Pass the raw bytes for direct comparison
    textures: &mut HashMap<u32, LandTexture>,
    landscapes: &mut HashMap<(i32, i32), Landscape>,
) {
    if *rec_name_bytes == *b"LTEX" {
        let tex = LandTexture::load(reader).unwrap().0;
        println!("tex: {:?}", tex);
        textures.insert(tex.index, tex);
    }

    if *rec_name_bytes == *b"LAND" {
        let mut landscape = Landscape::load(reader).unwrap();
        if let Some(indices) = landscape.texture_indices.as_mut() {
            for index in indices.iter_mut() {
                // we want to wrap around so 0 values become u16::max, so we know which would have
                // been 0 but still get to use a 0 index array.
                *index = index.wrapping_sub(1);
            }
        }
        landscapes.insert((landscape.grid_x, landscape.grid_y), landscape.clone());
        if landscape.grid_x == -2 && landscape.grid_y == -9 {
            println!("found land");
        }
    }
}
