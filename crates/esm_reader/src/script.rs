// src/script.rs

 // Name is FixedString<4>, used for subrecord matching
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId; // Scripts are identified by RefId (string-based)

// This struct represents the 20-byte data block within SCHD, after the 32-byte name.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
struct ScriptHeaderCountsAndSizes {
    pub num_shorts: u32,
    pub num_longs: u32,
    pub num_floats: u32,
    pub script_data_size: u32,  // Size of SCDT
    pub string_table_size: u32, // Size of SCVR
} // 5 * 4 = 20 bytes

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Script {
    pub id: RefId, // Script ID, from first 32 bytes of SCHD
    pub num_shorts: u32,
    pub num_longs: u32,
    pub num_floats: u32,
    pub script_data_size: u32,  // From SCHD, expected size of SCDT
    pub string_table_size: u32, // From SCHD, expected size of SCVR

    pub var_names: Vec<String>,         // From SCVR subrecord
    pub script_text: Option<String>,    // From SCTX subrecord
    pub compiled_data: Option<Vec<u8>>, // From SCDT subrecord
}

impl Script {
    pub fn new() -> Self {
        Default::default()
    }

    /// Parses a block of null-terminated strings from SCVR data.
    /// Handles '\r' as a potential terminator as well, like OpenMW.
    fn parse_scvr_string_table(
        mut data: Vec<u8>,
        total_expected_vars: usize,
    ) -> Result<Vec<String>, String> {
        let mut names = Vec::with_capacity(total_expected_vars);

        if data.is_empty() {
            if total_expected_vars > 0 {
                // OpenMW logs a warning but continues if SCVR is empty but vars were expected.
                // For strictness, we could error. For compatibility, return empty or partially filled.
                // For now, let's match OpenMW's behavior of trying to parse what's there.
            }
            return Ok(names);
        }

        // Replace '\r' with '\0' as OpenMW does to handle both terminators
        for byte in data.iter_mut() {
            if *byte == b'\r' {
                *byte = b'\0';
            }
        }
        // Ensure last char is null for safe splitting (OpenMW does this)
        if data.last() != Some(&0) {
            data.push(0);
        }

        let mut current_start = 0;
        for i in 0..data.len() {
            if data[i] == 0 {
                if current_start < i {
                    // Avoid empty strings from consecutive nulls, though OpenMW might keep them
                    match String::from_utf8(data[current_start..i].to_vec()) {
                        Ok(s) => names.push(s),
                        Err(e) => return Err(format!("Invalid UTF-8 in SCVR string: {}", e)),
                    }
                } else if current_start == i && names.len() < total_expected_vars {
                    // Handle potentially empty string if it's an expected variable
                    names.push(String::new());
                }
                current_start = i + 1;
                if names.len() == total_expected_vars {
                    break; // Found all expected names
                }
            }
        }

        // If not all expected vars were found (e.g., due to early end of data or fewer null terms)
        // OpenMW resizes the mVarNames list at the end if str > tmpEnd.
        // Here, we've pushed what we could find up to total_expected_vars.
        // If names.len() < total_expected_vars, some vars will be missing from the list.
        // This can be handled by the caller if necessary (e.g., by filling with empty strings).

        Ok(names)
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut script = Script::new();
        let mut schd_loaded_data_tuple: Option<(RefId, ScriptHeaderCountsAndSizes)> = None;
        let mut sctx_found = false;
        let mut scdt_actual_size_opt: Option<u32> = None; // Renamed to avoid confusion, made Option

        // Corrected: Use .as_str()
        let record_id_for_errors_cow = reader.m_ctx.rec_name.to_string_lossy();
        let record_id_for_errors = record_id_for_errors_cow.as_str();

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"SCHD" => {
                    reader.get_sub_header()?;
                    let expected_schd_total_size =
                        32 + std::mem::size_of::<ScriptHeaderCountsAndSizes>() as u32;
                    if reader.get_sub_size() != expected_schd_total_size {
                        return Err(reader.report_sub_size_mismatch(
                            expected_schd_total_size,
                            reader.get_sub_size(),
                        ));
                    }

                    let mut id_bytes = [0u8; 32];
                    reader.get_exact(&mut id_bytes)?;
                    let id_len = id_bytes.iter().position(|&b| b == 0).unwrap_or(32);
                    let id_string = String::from_utf8_lossy(&id_bytes[..id_len]).to_string();
                    let script_id_from_schd = RefId::string_ref_id(&id_string);

                    let counts_data = reader.get_t::<ScriptHeaderCountsAndSizes>()?;

                    schd_loaded_data_tuple = Some((script_id_from_schd, counts_data));
                }
                b"SCVR" => {
                    // Corrected: Remove 'ref' from header_counts
                    let (_id_from_schd_ref_val, header_counts) =
                        schd_loaded_data_tuple.as_ref().ok_or_else(|| {
                            reader.fail(&format!(
                                "SCPT '{}': SCVR found before SCHD",
                                record_id_for_errors
                            ))
                        })?;

                    reader.get_sub_header()?;
                    let scvr_data_size_from_sub = reader.get_sub_size();

                    // Copy packed field before using in format!
                    let expected_string_table_size = header_counts.string_table_size;
                    if scvr_data_size_from_sub < expected_string_table_size {
                        return Err(reader.fail(&format!(
                            "SCPT '{}': SCVR subrecord data size ({}) is smaller than string_table_size ({}) declared in SCHD", 
                            _id_from_schd_ref_val.to_debug_string(), scvr_data_size_from_sub, expected_string_table_size
                        )));
                    }

                    if expected_string_table_size > 0 {
                        let mut buffer = vec![0u8; expected_string_table_size as usize];
                        reader.get_exact(&mut buffer)?;
                        let total_vars = header_counts.num_shorts
                            + header_counts.num_longs
                            + header_counts.num_floats;

                        script.var_names =
                            Self::parse_scvr_string_table(buffer, total_vars as usize).map_err(
                                |e| {
                                    reader.fail(&format!(
                                        "SCPT '{}': Error parsing SCVR string table: {}",
                                        _id_from_schd_ref_val.to_debug_string(),
                                        e
                                    ))
                                },
                            )?;
                    }

                    if scvr_data_size_from_sub > expected_string_table_size {
                        reader.skip(scvr_data_size_from_sub - expected_string_table_size)?;
                    }
                }
                b"SCTX" => {
                    script.script_text = Some(reader.get_h_string()?);
                    sctx_found = true;
                }
                b"SCDT" => {
                    // Corrected: Remove 'ref' from header_counts
                    let (_id_from_schd_ref_val, header_counts) =
                        schd_loaded_data_tuple.as_ref().ok_or_else(|| {
                            reader.fail(&format!(
                                "SCPT '{}': SCDT found before SCHD",
                                record_id_for_errors
                            ))
                        })?;

                    reader.get_sub_header()?;
                    let scdt_sub_size_actual = reader.get_sub_size();
                    scdt_actual_size_opt = Some(scdt_sub_size_actual);

                    if scdt_sub_size_actual != header_counts.script_data_size {
                        // This is a warning in OpenMW, not a fatal error.
                        // println!("[SCPT WARNING] Record '{}': SCDT size in SCHD ({}) != actual SCDT subrecord size ({}). Using actual size.",
                        //     _id_from_schd_ref_val.to_debug_string(), header_counts.script_data_size, scdt_sub_size_actual);
                    }

                    if scdt_sub_size_actual > 0 {
                        let mut buffer = vec![0u8; scdt_sub_size_actual as usize];
                        reader.get_exact(&mut buffer)?;
                        script.compiled_data = Some(buffer);
                    } else {
                        script.compiled_data = Some(Vec::new());
                    }
                }
                b"NAME" => {
                    reader.skip_h_sub()?;
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if let Some((id_from_schd, header_data)) = schd_loaded_data_tuple {
            script.id = id_from_schd;
            script.num_shorts = header_data.num_shorts;
            script.num_longs = header_data.num_longs;
            script.num_floats = header_data.num_floats;
            script.script_data_size = header_data.script_data_size; // Ensure this matches Script struct field
            script.string_table_size = header_data.string_table_size; // Ensure this matches Script struct field
        } else {
            return Err(reader.fail(&format!(
                "SCPT record for potential ID '{}' missing SCHD subrecord",
                record_id_for_errors
            )));
        }

        // Use the variable to silence the warning if desired, or remove scdt_actual_size_opt if not used further
        if let Some(actual_size) = scdt_actual_size_opt {
            if actual_size != script.script_data_size {
                // This is where the OpenMW warning about SCDT size mismatch would fit.
                // Using the variable here to avoid the "assigned but never read" warning if you uncomment.
                //  println!(
                //    "[SCPT VALIDATION] SCHD expected SCDT size: {}, actual SCDT size: {}",
                //  script.script_data_size, actual_size
                //);
            }
        } else if script.script_data_size > 0 { // SCHD declared SCDT data, but no SCDT subrecord found
            // This case means SCDT was expected but not found.
            // println!("[SCPT WARNING] Record '{}': SCHD declared script_data_size {} but no SCDT subrecord found.",
            //     script.id.to_debug_string(), script.script_data_size);
        }
        if !sctx_found && script.script_text.is_none() {
            return Err(reader.fail(&format!(
                "SCPT record '{}' missing SCTX (script text) subrecord",
                script.id.to_debug_string()
            )));
        }

        Ok(script)
    }
}
