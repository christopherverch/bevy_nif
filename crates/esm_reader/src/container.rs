// src/container.rs

use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContItem {
    pub count: i32,
    pub item_id: RefId,
}

// ContainerData struct is removed as CNDT and FLAG are separate subrecords.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Container {
    pub id: RefId,                 // From NAME subrecord (Record ID)
    pub model: Option<String>,     // From MODL subrecord
    pub full_name: Option<String>, // From FNAM subrecord (Displayable name, optional)

    pub weight: Option<f32>, // From CNDT subrecord (float, 4 bytes)
    pub flags: Option<u32>,  // From FLAG subrecord (u32, 4 bytes)
    // Flags: 0x0008=Organic, 0x0010=Respawns, 0x0020=Locked (OpenMW notes 0x8 as "Unknown" but usually means "Organic" or "Can't be picked up")
    pub inventory: Vec<ContItem>, // From NPCO subrecords
    pub script_id: Option<RefId>, // From SCRI subrecord (optional)
}

impl Container {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut cont = Container::new();
        let mut id_found = false;
        let mut cndt_found = false; // For CNDT (weight)
        let mut flag_sub_found = false; // For FLAG subrecord

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    cont.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    cont.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    cont.full_name = Some(reader.get_h_string()?);
                }
                b"CNDT" => {
                    // Container Weight Data (float, 4 bytes)
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    cont.weight = Some(reader.get_t::<f32>()?);
                    cndt_found = true;
                }
                b"FLAG" => {
                    // Container Flags (u32, 4 bytes)
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    cont.flags = Some(reader.get_t::<u32>()?);
                    flag_sub_found = true;
                }
                b"NPCO" => {
                    reader.get_sub_header()?;
                    let total_npco_size = reader.get_sub_size();

                    if total_npco_size < 4 {
                        return Err(reader.fail("NPCO subrecord in CONT too small for count"));
                    }
                    let count = reader.get_t::<i32>()?;
                    let item_id_str_size = total_npco_size.saturating_sub(4);

                    let item_id_str = reader.get_string_view_internal(item_id_str_size as usize)?;

                    cont.inventory.push(ContItem {
                        count,
                        item_id: RefId::string_ref_id(&item_id_str),
                    });
                }
                b"SCRI" => {
                    cont.script_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || cont.id.empty() {
            return Err(reader.fail("CONT record missing mandatory NAME subrecord for its ID"));
        }
        // OpenMW requires CNDT (hasWeight) and FLAG (hasFlags) to be present unless deleted.
        if !cndt_found {
            return Err(reader.fail(&format!(
                "CONT record '{}' missing CNDT (weight) subrecord",
                cont.id.to_debug_string()
            )));
        }
        if !flag_sub_found {
            return Err(reader.fail(&format!(
                "CONT record '{}' missing FLAG (flags) subrecord",
                cont.id.to_debug_string()
            )));
        }

        Ok(cont)
    }
}

