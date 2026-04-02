// src/main.rs

use esm_parser_lib::{EsmReader, Header};
use record_handler::try_parse_and_print_record;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use to_utf8::Utf8Encoder;

mod header_utils;
mod record_handler;
#[test]
fn test_esm_file_parsing() {
    println!("ESM Reader Test Program");

    let file_path_str = "Morrowind.esm";
    let file_path = Path::new(file_path_str);

    if !file_path.exists() {
        eprintln!(
            "Error: File not found at '{}'. Please update the path in tests.rs.",
            file_path.display()
        );
        return;
    }

    let encoder = Utf8Encoder::new(to_utf8::FromType::Windows1252);
    let mut reader = EsmReader::new(encoder);

    println!("\nAttempting to open: {}", file_path.display());
    let start_time = Instant::now();

    match reader.open(file_path) {
        Ok(_) => {
            let duration = start_time.elapsed();
            println!(
                "Successfully opened '{}' in {:?}",
                file_path.display(),
                duration
            );
            let header: &Header = reader.get_header();
            header_utils::print_header_information(header);

            println!("\n--- Reading Record(s) ---");
            let parsed_counts: HashMap<String, usize> = HashMap::new();
            let records_to_show_each = 1;

            let target_record_types = [
                // "GMST", "BSGN", "CONT", "FACT", "LOCK", "PROB", "GLOB", "SKIL", "CLAS", "RACE",
                //"SOUN", "REGN", "MGEF", "SPEL", "BOOK", , "ARMO", "MISC", "LIGH", "CREA",
                //"INGR", "CLOT", "APPA", "ALCH", "LEVI", "STAT", "DOOR", "LEVC", "ENCH", "ACTI",
                //"BODY", "REPA", "PGRD", "CELL", "NPC_", "SNDG", "AMMO", "LVLN", "SCPT", "DIAL",
                //"INFO", "LTEX", "WAYP", "LAND", "KEYM",
                // "LAND", "LTEX", "WEAP",
                "BODY",
            ];
            'record_loop: while reader.has_more_recs().expect("Failed to read rec") {
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
                                        record_id_for_messages, // Pass &str for messages
                                        &record_type_for_map_key, // Pass owned String for map key usage if needed by handler
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
            if reader.has_more_recs().expect("failed to read recs") {
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
}
