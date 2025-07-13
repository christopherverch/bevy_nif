// src/clothing.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct ClothingData {
    pub clothing_type: i32, // Enum: 0=Pants, 1=Shoes, ..., 10=Robe etc. (OpenMW uses int)
    pub weight: f32,
    pub value: i16,       // OpenMW uses short
    pub enchant_pts: i16, // OpenMW uses short
} // Total: 4 (type) + 4 (weight) + 2 (value) + 2 (enchant_pts) = 12 bytes. Matches OpenMW CTDT.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Clothing {
    pub id: RefId,                  // From NAME subrecord
    pub model: Option<String>,      // From MODL subrecord (world model)
    pub full_name: Option<String>,  // From FNAM subrecord (Displayable name)
    pub data: Option<ClothingData>, // From CTDT subrecord
    pub icon: Option<String>,       // From ITEX subrecord

    pub part_index: Option<u8>, // From INDX (primary body part slot this clothing uses)
    pub male_biped_models: Vec<String>, // From BNAM subrecords (male model for body part)
    pub female_biped_models: Vec<String>, // From CNAM subrecords (female model for body part)

    pub script_id: Option<RefId>,      // From SCRI subrecord (optional)
    pub enchantment_id: Option<RefId>, // From ENAM subrecord (optional)
}

impl Clothing {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut clot = Clothing::new();
        let mut id_found = false;
        let mut ctdt_found = false;
        let mut fnam_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    clot.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    clot.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    clot.full_name = Some(reader.get_h_string()?);
                    fnam_found = true;
                }
                b"CTDT" => {
                    // Clothing Data
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<ClothingData>() as u32; // 12 bytes
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    let mut ctdt_data = ClothingData::default();
                    ctdt_data.clothing_type = reader.get_t::<i32>()?;
                    ctdt_data.weight = reader.get_t::<f32>()?;
                    ctdt_data.value = reader.get_t::<i16>()?;
                    ctdt_data.enchant_pts = reader.get_t::<i16>()?;
                    clot.data = Some(ctdt_data);
                    ctdt_found = true;
                }
                b"ITEX" => {
                    clot.icon = Some(reader.get_h_string()?);
                }
                b"INDX" => {
                    // Body part index
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 1 {
                        return Err(reader.report_sub_size_mismatch(1, reader.get_sub_size()));
                    }
                    // Similar to ARMO, store the first/last INDX encountered.
                    // OpenMW's PartReference system is more complex for grouping.
                    if clot.part_index.is_none() {
                        clot.part_index = Some(reader.get_t::<u8>()?);
                    } else {
                        let _ = reader.get_t::<u8>()?; // Read and discard if already set
                    }
                }
                b"BNAM" => {
                    // Male biped model part filename
                    clot.male_biped_models.push(reader.get_h_string()?);
                }
                b"CNAM" => {
                    // Female biped model part filename
                    clot.female_biped_models.push(reader.get_h_string()?);
                }
                b"SCRI" => {
                    clot.script_id = Some(reader.get_ref_id()?);
                }
                b"ENAM" => {
                    clot.enchantment_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || clot.id.empty() {
            return Err(reader.fail("CLOT record missing mandatory NAME subrecord for its ID"));
        }
        if !fnam_found {
            return Err(reader.fail(&format!(
                "CLOT record '{}' missing FNAM subrecord",
                clot.id.to_debug_string()
            )));
        }
        if !ctdt_found {
            return Err(reader.fail(&format!(
                "CLOT record '{}' missing CTDT subrecord",
                clot.id.to_debug_string()
            )));
        }

        Ok(clot)
    }
}
