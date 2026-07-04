// src/main.rs

use esm_parser_lib::EsmReader;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use to_utf8::Utf8Encoder;

use crate::record_handler::process_record;

mod header_utils;
mod record_handler;
fn run_esm_test<F>(mut test_logic: F)
where
    F: FnMut(&mut EsmReader),
{
    let file_path = Path::new("Morrowind.esm");

    if !file_path.exists() {
        panic!("Morrowind.esm not found at {:?}.", file_path);
    }

    let encoder = Utf8Encoder::new(to_utf8::FromType::Windows1252);
    let mut reader = EsmReader::new(encoder);

    println!("\nOpening: {}", file_path.display());
    let start_time = Instant::now();

    reader.open(file_path).expect("Failed to open ESM");

    let header = reader.get_header();
    header_utils::print_header_information(header);

    // Call the specific test logic (the loop)
    test_logic(&mut reader);

    println!("\nFinished in {:?}", start_time.elapsed());
    reader.close();
}
#[test]
fn test_exhaustive_parse() {
    run_esm_test(|reader| {
        let mut total = 0;
        while reader.has_more_recs().unwrap() {
            let name = reader.read_rec_name().unwrap();
            reader.read_rec_header().unwrap();

            // Use the dispatcher with an empty closure (will be skipped)
            process_record(reader, &name.data, |_, _| {}).unwrap();

            total += 1;
            if total % 50000 == 0 {
                println!("Parsed {}...", total);
            }
        }
        println!("Verified {} total records.", total);
    });
}
#[test]
fn test_print_samples() {
    run_esm_test(|reader| {
        let mut parsed_counts: HashMap<String, usize> = HashMap::new();
        let target_types = [
            // "GMST", "BSGN", "CONT", "FACT", "LOCK", "PROB", "GLOB", "SKIL", "CLAS", "RACE",
            //"SOUN", "REGN", "MGEF", "SPEL", "BOOK", , "ARMO", "MISC", "LIGH", "CREA",
            //"INGR", "CLOT", "APPA", "ALCH", "LEVI", "STAT", "DOOR", "LEVC", "ENCH", "ACTI",
            //"BODY", "REPA", "PGRD", "CELL", "NPC_", "SNDG", "AMMO", "LVLN", "SCPT", "DIAL",
            //"INFO", "LTEX", "WAYP", "LAND", "KEYM",
            // "LAND", "LTEX", "WEAP",
            "SPEL",
        ];
        while reader.has_more_recs().unwrap() {
            let name = reader.read_rec_name().unwrap();
            let name_str = name.to_string_lossy();
            reader.read_rec_header().unwrap();

            let count = *parsed_counts.get(&name_str).unwrap_or(&0);

            if target_types.contains(&name_str.as_str()) && count < 100 {
                // Pass a printing closure to the dispatcher
                process_record(reader, &name.data, |tag, data| {
                    println!("[{}] {:#?}", std::str::from_utf8(tag).unwrap(), data);
                })
                .unwrap();

                parsed_counts.insert(name_str, count + 1);
            } else {
                reader.skip_record().unwrap();
            }
        }
    });
}
