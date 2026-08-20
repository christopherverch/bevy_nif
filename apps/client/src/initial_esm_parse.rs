use crate::defs::{ESM_READER, LandTextureDefinitions};
use crate::landscape::ALL_LANDSCAPES;
use bevy::prelude::*;
use esm_parser_lib::types::cell::Cell;
use esm_parser_lib::types::static_object::Static;
use esm_parser_lib::{
    EsmError, EsmReader, Header, LandTexture, Landscape, Load, MAGIC_EFFECT_ID_TO_INDEX, RefId,
    Weapon,
    types::{
        bodypart::Bodypart,
        flags::ObjectFlags,
        gmst::GameSetting,
        magic_effect::MagicEffect,
        npc::Npc,
        race::Race,
        skill::{MagicSchool, Skill, SkillIndex},
        spell::Spell,
    },
};
use std::sync::RwLock;
use std::{collections::HashMap, path::Path, time::Instant};
use to_utf8::Utf8Encoder;

use esm_store::{EsmStore, Store};

pub fn populate_esm_resources(mut commands: Commands) {
    if let Ok((reader, landscapes, land_textures, esm_store)) = run_initial_parse() {
        let _ = ESM_READER.set(RwLock::new(reader));
        let _ = ALL_LANDSCAPES.set(RwLock::new(landscapes));
        commands.insert_resource(LandTextureDefinitions(land_textures));
        commands.insert_resource(esm_store);
    } else {
        panic!("Failed to parse esm!");
    };
}
pub fn run_initial_parse() -> Result<
    (
        EsmReader,
        HashMap<(i32, i32), Landscape>,
        HashMap<u32, LandTexture>,
        EsmStore,
    ),
    EsmError,
> {
    let mut land_textures = HashMap::new();
    let mut landscapes = HashMap::new();

    let encoder = Utf8Encoder::new(to_utf8::FromType::Windows1252);
    let mut reader = EsmReader::new(encoder);
    reader.set_encoder(Utf8Encoder::new(to_utf8::FromType::Windows1252));

    let file_path_str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Morrowind.esm");
    let file_path = Path::new(file_path_str);
    println!("\nAttempting to open: {}", file_path.display());
    let start_time = Instant::now();
    let mut file_paths = Vec::new();
    file_paths.push(file_path);
    let file_path_str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/data/Data Files/Bloodmoon.esm"
    );
    let file_path = Path::new(file_path_str);
    file_paths.push(file_path);
    let file_path_str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/data/Data Files/Tribunal.esm"
    );
    let file_path = Path::new(file_path_str);
    file_paths.push(file_path);
    let file_path_str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/test.esp");
    let file_path = Path::new(file_path_str);
    file_paths.push(file_path);
    let mut esm_store = EsmStore::default();
    let mut previous_loop_tex_amount = 0;
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
                    //"INGR", "CLOT", "APPA", "ALCH", "LEVI",  "DOOR", "LEVC", "ENCH", "ACTI",
                    //"BODY", "REPA", "PGRD",  "NPC_", "SNDG", "AMMO", "LVLN", "SCPT", "DIAL", "INFO", "LTEX", "WAYP", "LAND", "KEYM",
                    "LAND", "LTEX", "WEAP", "RACE", "BODY", "NPC_", "SPEL", "MGEF", "GMST", "SKIL",
                    "STAT", "CELL",
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
                                        load_record(
                                            &mut reader,
                                            &mut esm_store,
                                            &record_name_bytes, // Pass the raw bytes for matching
                                            &mut land_textures,
                                            &mut landscapes,
                                            previous_loop_tex_amount,
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
                return Err(EsmError::Custom(format!(
                    "Morrowind.esm not found in {:?}",
                    file_path
                )));
            }
        }
        previous_loop_tex_amount = land_textures.len() as u32;
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
    finish_magic_effect_setup(&mut esm_store.magic_effects, &esm_store.gamesettings);
    finish_skill_setup(&mut esm_store.skills, &esm_store.gamesettings);
    Ok((reader, landscapes, land_textures, esm_store))
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

pub fn load_record(
    reader: &mut EsmReader,
    esm_store: &mut EsmStore,
    rec_name_bytes: &[u8; 4], // Pass the raw bytes for direct comparison
    textures: &mut HashMap<u32, LandTexture>,
    landscapes: &mut HashMap<(i32, i32), Landscape>,
    previous_loop_tex_amount: u32,
) {
    if *rec_name_bytes == *b"LTEX" {
        match LandTexture::load(reader) {
            Ok(tex) => {
                if tex.flags.contains(ObjectFlags::DELETED) {
                    // Remove if a mod deleted it.
                    textures.remove(&tex.index);
                } else {
                    textures.insert(tex.index + previous_loop_tex_amount, tex);
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
                    if let Some(indices) = landscape.texture_indices.as_mut() {
                        // Subtract 1 because 0 is the default texture, which we add at the end
                        for index in indices.iter_mut() {
                            *index = index.wrapping_sub(1);
                            // So bloodmoon/tribunal don't override base mw textures

                            if *index < u16::MAX {
                                *index += previous_loop_tex_amount as u16;
                            }
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
                    esm_store.weapons.remove_static(&weapon.id);
                } else {
                    esm_store.weapons.insert_static(weapon.id.clone(), weapon);
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
                    esm_store.races.remove_static(&race.id);
                } else {
                    esm_store.races.insert_static(race.id.clone(), race);
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
                    esm_store.bodyparts.remove_static(&bodypart.id);
                } else {
                    esm_store
                        .bodyparts
                        .insert_static(bodypart.id.clone(), bodypart);
                }
            }
            Err(e) => {
                eprintln!("Failed to parse BODY record: {}", e);
            }
        }
    }
    if *rec_name_bytes == *b"NPC_" {
        let mut is_deleted = false;
        match Npc::load(reader, &mut is_deleted) {
            Ok(npc) => {
                if is_deleted {
                    esm_store.npcs.remove_static(&npc.id);
                } else {
                    esm_store.npcs.insert_static(npc.id.clone(), npc);
                }
            }
            Err(e) => {
                eprintln!("Failed to parse NPC_ record: {}", e);
            }
        }
    }
    if *rec_name_bytes == *b"SPEL" {
        let mut is_deleted = false;
        match Spell::load(reader, &mut is_deleted) {
            Ok(spell) => {
                if is_deleted {
                    esm_store.spells.remove_static(&spell.id);
                } else {
                    esm_store.spells.insert_static(spell.id.clone(), spell);
                }
            }
            Err(e) => {
                eprintln!("Failed to parse SPEL record: {}", e);
            }
        }
    }
    if *rec_name_bytes == *b"MGEF" {
        let mut is_deleted = false;
        match MagicEffect::load(reader, &mut is_deleted) {
            Ok(magic_effect) => {
                if is_deleted {
                    esm_store.magic_effects.remove_static(&magic_effect.id);
                } else {
                    esm_store
                        .magic_effects
                        .insert_static(magic_effect.id.clone(), magic_effect);
                }
            }
            Err(e) => {
                eprintln!("Failed to parse MGEF record: {}", e);
            }
        }
    }
    if *rec_name_bytes == *b"GMST" {
        let mut is_deleted = false;
        match GameSetting::load(reader, &mut is_deleted) {
            Ok(gmst) => {
                if is_deleted {
                    esm_store.gamesettings.remove_static(&gmst.id);
                } else {
                    let hash_val = gmst.id.debug_hash();
                    println!(
                        "Inserting Record ID: {:?} | Hash: 0x{:X}",
                        gmst.id, hash_val
                    );
                    esm_store.gamesettings.insert_static(gmst.id.clone(), gmst);
                }
            }
            Err(e) => {
                eprintln!("Failed to parse GMST record: {}", e);
            }
        }
    }
    if *rec_name_bytes == *b"SKIL" {
        let mut is_deleted = false;
        match Skill::load(reader, &mut is_deleted) {
            Ok(skill) => {
                if is_deleted {
                    esm_store.skills.remove_static(&skill.id);
                } else {
                    esm_store.skills.insert_static(skill.id.clone(), skill);
                }
            }
            Err(e) => {
                eprintln!("Failed to parse SKIL record: {}", e);
            }
        }
    }
    if *rec_name_bytes == *b"CELL" {
        let mut is_deleted = false;
        match Cell::load(reader, &mut is_deleted, false) {
            Ok(mut cell) => {
                cell.post_load(reader).unwrap();
                if is_deleted {
                    esm_store.cells.remove_static(&cell.id);
                } else {
                    // TODO:: handle expansions, should put these in separate maps by esm or
                    // something, for now just don't replace old ones
                    if !esm_store.cells.get(&cell.id).is_some() {
                        esm_store.cells.insert_static(cell.id.clone(), cell);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to parse SKIL record: {}", e);
            }
        }
    }
    if *rec_name_bytes == *b"STAT" {
        let mut is_deleted = false;
        match Static::load(reader, &mut is_deleted) {
            Ok(stat) => {
                if is_deleted {
                    esm_store.statics.remove_static(&stat.id);
                } else {
                    esm_store.statics.insert_static(stat.id.clone(), stat);
                }
            }
            Err(e) => {
                eprintln!("Failed to parse SKIL record: {}", e);
            }
        }
    }
}
fn finish_magic_effect_setup(
    magic_effects: &mut Store<MagicEffect>,
    gamesettings: &Store<GameSetting>,
) {
    for effect in magic_effects.static_rec.values_mut() {
        // Step A: Map RefId to Index (O(1))
        if let Some(&index) = MAGIC_EFFECT_ID_TO_INDEX.get(&effect.id) {
            // Step B: Map Index to GMST Key string (e.g., "sMagicWaterBreathingName")
            let gmst_key = MagicEffect::index_to_gmst_str(index);

            // Step C: Transparent Search in GMST Store (O(1))
            if let Some(gmst_record) = gamesettings.search(gmst_key) {
                // Step D: Update the effect name
                effect.name = gmst_record.value.0.as_str().unwrap_or_default().to_string();
            }
        }
    }
}
fn finish_skill_setup(skills: &mut Store<Skill>, gamesettings: &Store<GameSetting>) {
    for skill in skills.static_rec.values_mut() {
        // Map RefId to Index
        let Some(index) = SkillIndex::refid_to_skillindex(&skill.id) else {
            continue;
        };
        let meta = &SKILL_METADATA[index];

        // Set Name from GMST
        if let Some(gmst) = gamesettings.search(meta.gmst_name) {
            skill.name = gmst.value.0.as_str().unwrap_or_default().to_string();
        }

        // Set Icon and Werewolf Value
        skill.icon = meta.icon.to_string();
        if let Some(gmst) = gamesettings.search(meta.werewolf_gmst) {
            skill.werewolf_value = gmst.value.0.as_float();
        }

        // Set up the magic school refids
        if !meta.school_id.is_empty() {
            let id = meta.school_id;
            let school = skill.school.get_or_insert_with(MagicSchool::default);

            // Sound IDs
            school.area_sound = RefId::from_str(&format!("{} area", id));
            school.bolt_sound = RefId::from_str(&format!("{} bolt", id));
            school.cast_sound = RefId::from_str(&format!("{} cast", id));
            school.hit_sound = RefId::from_str(&format!("{} hit", id));
            school.failure_sound = RefId::from_str(&format!("Spell Failure {}", id));

            // School Name (e.g., "sSchooldestruction")
            let school_name_key = format!("sSchool{}", id);
            if let Some(gmst) = gamesettings.search(&school_name_key) {
                school.name = gmst.value.0.as_str().unwrap_or_default().to_string();
            }

            // AutoCalc Max (e.g., "iAutoSpelldestructionMax")
            let autocalc_key = format!("iAutoSpell{}Max", id);
            if let Some(gmst) = gamesettings.search(&autocalc_key) {
                school.auto_calc_max = gmst.value.0.as_float() as i32;
            }
        }
    }
}
struct SkillMetadata {
    gmst_name: &'static str,
    icon: &'static str,
    werewolf_gmst: &'static str,
    school_id: &'static str, // Empty string if no school
}

const SKILL_METADATA: [SkillMetadata; 27] = [
    SkillMetadata {
        gmst_name: "sSkillBlock",
        icon: "icons\\k\\combat_block.dds",
        werewolf_gmst: "fWerewolfBlock",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillArmorer",
        icon: "icons\\k\\combat_armor.dds",
        werewolf_gmst: "fWerewolfArmorer",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillMediumarmor",
        icon: "icons\\k\\combat_mediumarmor.dds",
        werewolf_gmst: "fWerewolfMediumarmor",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillHeavyarmor",
        icon: "icons\\k\\combat_heavyarmor.dds",
        werewolf_gmst: "fWerewolfHeavyarmor",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillBluntweapon",
        icon: "icons\\k\\combat_blunt.dds",
        werewolf_gmst: "fWerewolfBluntweapon",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillLongblade",
        icon: "icons\\k\\combat_longblade.dds",
        werewolf_gmst: "fWerewolfLongblade",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillAxe",
        icon: "icons\\k\\combat_axe.dds",
        werewolf_gmst: "fWerewolfAxe",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillSpear",
        icon: "icons\\k\\combat_spear.dds",
        werewolf_gmst: "fWerewolfSpear",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillAthletics",
        icon: "icons\\k\\combat_athletics.dds",
        werewolf_gmst: "fWerewolfAthletics",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillEnchant",
        icon: "icons\\k\\magic_enchant.dds",
        werewolf_gmst: "fWerewolfEnchant",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillDestruction",
        icon: "icons\\k\\magic_destruction.dds",
        werewolf_gmst: "fWerewolfDestruction",
        school_id: "destruction",
    },
    SkillMetadata {
        gmst_name: "sSkillAlteration",
        icon: "icons\\k\\magic_alteration.dds",
        werewolf_gmst: "fWerewolfAlteration",
        school_id: "alteration",
    },
    SkillMetadata {
        gmst_name: "sSkillIllusion",
        icon: "icons\\k\\magic_illusion.dds",
        werewolf_gmst: "fWerewolfIllusion",
        school_id: "illusion",
    },
    SkillMetadata {
        gmst_name: "sSkillConjuration",
        icon: "icons\\k\\magic_conjuration.dds",
        werewolf_gmst: "fWerewolfConjuration",
        school_id: "conjuration",
    },
    SkillMetadata {
        gmst_name: "sSkillMysticism",
        icon: "icons\\k\\magic_mysticism.dds",
        werewolf_gmst: "fWerewolfMysticism",
        school_id: "mysticism",
    },
    SkillMetadata {
        gmst_name: "sSkillRestoration",
        icon: "icons\\k\\magic_restoration.dds",
        werewolf_gmst: "fWerewolfRestoration",
        school_id: "restoration",
    },
    SkillMetadata {
        gmst_name: "sSkillAlchemy",
        icon: "icons\\k\\magic_alchemy.dds",
        werewolf_gmst: "fWerewolfAlchemy",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillUnarmored",
        icon: "icons\\k\\magic_unarmored.dds",
        werewolf_gmst: "fWerewolfUnarmored",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillSecurity",
        icon: "icons\\k\\stealth_security.dds",
        werewolf_gmst: "fWerewolfSecurity",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillSneak",
        icon: "icons\\k\\stealth_sneak.dds",
        werewolf_gmst: "fWerewolfSneak",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillAcrobatics",
        icon: "icons\\k\\stealth_acrobatics.dds",
        werewolf_gmst: "fWerewolfAcrobatics",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillLightarmor",
        icon: "icons\\k\\stealth_lightarmor.dds",
        werewolf_gmst: "fWerewolfLightarmor",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillShortblade",
        icon: "icons\\k\\stealth_shortblade.dds",
        werewolf_gmst: "fWerewolfShortblade",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillMarksman",
        icon: "icons\\k\\stealth_marksman.dds",
        werewolf_gmst: "fWerewolfMarksman",
        school_id: "",
    },
    // "Mercantile"! >_<
    SkillMetadata {
        gmst_name: "sSkillMercantile",
        icon: "icons\\k\\stealth_mercantile.dds",
        werewolf_gmst: "fWerewolfMerchantile",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillSpeechcraft",
        icon: "icons\\k\\stealth_speechcraft.dds",
        werewolf_gmst: "fWerewolfSpeechcraft",
        school_id: "",
    },
    SkillMetadata {
        gmst_name: "sSkillHandtohand",
        icon: "icons\\k\\stealth_handtohand.dds",
        werewolf_gmst: "fWerewolfHandtohand",
        school_id: "",
    },
];
