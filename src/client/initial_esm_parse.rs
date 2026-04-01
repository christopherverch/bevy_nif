use bevy::prelude::*;
use esm_parser_lib::{
    EsmError, EsmReader, Header, LandTexture, Landscape, Load, RefId, Weapon,
    types::{bodypart::Bodypart, flags::ObjectFlags, race::Race},
};
use std::{collections::HashMap, path::Path, time::Instant};
use tes3mp_rust::defs::{
    AllLandscapes, BodypartDefinitions, EsmReaderResource, LandTextureDefinitions, RaceDefinitions,
    WeaponDefinitions,
};
use to_utf8::Utf8Encoder;

pub fn populate_esm_resources(mut commands: Commands) {
    if let Ok((reader, landscapes, land_textures, weapons, races, bodyparts)) = run_initial_parse()
    {
        commands.insert_resource(EsmReaderResource(reader));
        commands.insert_resource(AllLandscapes(landscapes));
        commands.insert_resource(LandTextureDefinitions(land_textures));
        commands.insert_resource(WeaponDefinitions(weapons));
        commands.insert_resource(RaceDefinitions(races));
        commands.insert_resource(BodypartDefinitions(bodyparts));
    } else {
        panic!("Failed to parse esm!");
    };
}
pub fn run_initial_parse() -> Result<
    (
        EsmReader,
        HashMap<(i32, i32), Landscape>,
        HashMap<u32, LandTexture>,
        HashMap<RefId, Weapon>,
        HashMap<RefId, Race>,
        HashMap<RefId, Bodypart>,
    ),
    EsmError,
> {
    let mut land_textures = HashMap::new();
    let mut landscapes = HashMap::new();
    let mut weapons = HashMap::new();
    let mut races = HashMap::new();
    let mut bodyparts = HashMap::new();
    let encoder = Utf8Encoder::new(to_utf8::FromType::Windows1252);
    let mut reader = EsmReader::new(encoder);
    reader.set_encoder(Utf8Encoder::new(to_utf8::FromType::Windows1252));

    let file_path_str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Morrowind.esm");
    let file_path = Path::new(file_path_str);
    println!("\nAttempting to open: {}", file_path.display());
    let start_time = Instant::now();
    let mut file_paths = Vec::new();
    file_paths.push(file_path);
    let file_path_str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/test.esp");
    let file_path = Path::new(file_path_str);
    file_paths.push(file_path);
    for file_path in file_paths {
        match reader.open(file_path) {
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
                    //"BODY", "REPA", "PGRD", "CELL", "NPC_", "SNDG", "AMMO", "LVLN", "SCPT", "DIAL", "INFO", "LTEX", "WAYP", "LAND", "KEYM",
                    "LAND", "LTEX", "WEAP", "RACE", "BODY",
                ];
                'record_loop: while reader.has_more_recs()? {
                    let mut all_targets_done = true;
                    for rec_type_str_check in target_record_types {
                        if *parsed_counts.get(rec_type_str_check).unwrap_or(&0)
                            < records_to_show_each
                        {
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

                    match reader.read_rec_name() {
                        Ok(rec_name) => {
                            let record_name_bytes = rec_name.data;
                            let record_id_string_val = rec_name.to_string_lossy();
                            //println!("record name: {}", record_id_string_val);
                            let record_type_for_map_key = record_id_string_val.to_string();
                            let record_id_for_messages = record_id_string_val.as_str();

                            match reader.read_rec_header() {
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
                                            &mut weapons,
                                            &mut races,
                                            &mut bodyparts,
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
                if reader.has_more_recs()? {
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
    }
    // we have to insert this because it's always available as the "default texture"
    let default_texture = LandTexture {
        texture_file: "_land_default.tga".to_string(),
        // no idea what to put for id
        flags: ObjectFlags::empty(),
        id: RefId::default(),
        index: u16::MAX as u32,
    };
    land_textures.insert(default_texture.index, default_texture);
    Ok((reader, landscapes, land_textures, weapons, races, bodyparts))
}
pub fn print_header_information(header: &Header) {
    println!("\n--- Header Information ---");
    println!("  File Version (float): {}", header.esm_version_f());
    println!("  Author: '{}'", header.get_author());
    let desc_preview = header
        .get_description()
        .chars()
        .take(70)
        .collect::<String>();
    println!(
        "  Description: '{}'{}",
        desc_preview,
        if header.get_description().len() > 70 {
            "..."
        } else {
            ""
        }
    );
    println!("  Number of Records: {}", header.get_record_count());
    if !header.master_data.is_empty() {
        println!("  Master Files:");
        for master in &header.master_data {
            println!("    - Name: {}, Size: {}", master.name, master.size);
        }
    }
}

pub fn try_parse_and_print_record(
    reader: &mut EsmReader,
    rec_name_bytes: &[u8; 4], // Pass the raw bytes for direct comparison
    textures: &mut HashMap<u32, LandTexture>,
    landscapes: &mut HashMap<(i32, i32), Landscape>,
    weapons: &mut HashMap<RefId, Weapon>,
    races: &mut HashMap<RefId, Race>,
    bodyparts: &mut HashMap<RefId, Bodypart>,
) {
    if *rec_name_bytes == *b"LTEX" {
        match LandTexture::load(reader) {
            Ok(tex) => {
                if tex.flags.contains(ObjectFlags::DELETED) {
                    // Remove if a mod deleted it.
                    textures.remove(&tex.index);
                } else {
                    // Add or overwrite.
                    textures.insert(tex.index, tex);
                }
            }
            Err(e) => eprintln!("Failed to parse LTEX record: {}", e),
        }
    }

    if *rec_name_bytes == *b"LAND" {
        match Landscape::load(reader) {
            Ok(mut landscape) => {
                let key = (landscape.grid_x, landscape.grid_y);
                if landscape.flags.contains(ObjectFlags::DELETED) {
                    // Remove if a mod deleted it.
                    landscapes.remove(&key);
                } else {
                    // Your existing logic for texture indices
                    if let Some(indices) = landscape.texture_indices.as_mut() {
                        for index in indices.iter_mut() {
                            *index = index.wrapping_sub(1);
                        }
                    }
                    // Add or overwrite.
                    landscapes.insert(key, landscape);
                }
            }
            Err(e) => eprintln!("Failed to parse LAND record: {}", e),
        }
    }
    if *rec_name_bytes == *b"WEAP" {
        match Weapon::load(reader) {
            Ok(weapon) => {
                if weapon.flags.contains(ObjectFlags::DELETED) {
                    // A mod or patch has marked this weapon for deletion.
                    // Remove it from our map if a previous file added it.
                    println!("Deleting Weapon ID: {:?}", weapon.id);
                    weapons.remove(&weapon.id);
                } else {
                    // This is a new weapon or an override of an existing one.
                    weapons.insert(weapon.id.clone(), weapon);
                }
            }
            Err(e) => {
                eprintln!("Failed to parse WEAP record: {}", e);
            }
        }
    }
    if *rec_name_bytes == *b"RACE" {
        let mut is_deleted = false;
        match Race::load(reader, &mut is_deleted) {
            Ok(race) => {
                if is_deleted {
                    races.remove(&race.id);
                } else {
                    races.insert(race.id.clone(), race);
                }
            }
            Err(e) => {
                eprintln!("Failed to parse RACE record: {}", e);
            }
        }
    }
    if *rec_name_bytes == *b"BODY" {
        let mut is_deleted = false;
        match Bodypart::load(reader, &mut is_deleted) {
            Ok(bodypart) => {
                if is_deleted {
                    bodyparts.remove(&bodypart.id);
                } else {
                    bodyparts.insert(bodypart.id.clone(), bodypart);
                }
            }
            Err(e) => {
                eprintln!("Failed to parse RACE record: {}", e);
            }
        }
    }
}
