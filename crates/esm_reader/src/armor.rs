// src/armor.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct ArmorData {
    pub armor_type: i32,
    pub weight: f32,
    pub value: i32,
    pub health: i32,
    pub enchant_pts: i32,
    pub armor_rating: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BodyPartRef {
    // This might not be used if we simplify INDX/BNAM/CNAM handling
    pub part_index: u8,
    pub male_part_name: String,
    pub female_part_name: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Armor {
    pub id: RefId,
    pub model: Option<String>,
    pub full_name: Option<String>,
    pub data: Option<ArmorData>,
    pub icon: Option<String>,
    pub part_index: Option<u8>, // From INDX (primary part this armor covers)
    pub male_biped_models: Vec<String>, // From BNAM subrecords
    pub female_biped_models: Vec<String>, // From CNAM subrecords
    pub script_id: Option<RefId>,
    pub enchantment_id: Option<RefId>,
}

impl Armor {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut armor = Armor::new();
        let mut id_found = false;
        let mut adot_found = false;
        let mut fnam_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    armor.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    armor.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    armor.full_name = Some(reader.get_h_string()?);
                    fnam_found = true;
                }
                b"AODT" => {
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<ArmorData>() as u32;
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    armor.data = Some(reader.get_t::<ArmorData>()?);
                    adot_found = true;
                }
                b"ITEX" => {
                    armor.icon = Some(reader.get_h_string()?);
                }
                b"INDX" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 1 {
                        return Err(reader.report_sub_size_mismatch(1, reader.get_sub_size()));
                    }
                    let index_val = reader.get_t::<u8>()?;
                    if armor.part_index.is_none() {
                        // Store the first INDX encountered
                        armor.part_index = Some(index_val);
                    }
                    // If multiple INDX, subsequent ones are ignored for armor.part_index for simplicity.
                    // OpenMW might build a list of PartReference structs.
                }
                b"BNAM" => {
                    armor.male_biped_models.push(reader.get_h_string()?);
                }
                b"CNAM" => {
                    armor.female_biped_models.push(reader.get_h_string()?);
                }
                b"SCRI" => {
                    armor.script_id = Some(reader.get_ref_id()?);
                }
                b"ENAM" => {
                    armor.enchantment_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || armor.id.empty() {
            return Err(reader.fail("ARMO record missing mandatory NAME subrecord for its ID"));
        }
        if !fnam_found {
            return Err(reader.fail(&format!(
                "ARMO record '{}' missing FNAM subrecord",
                armor.id.to_debug_string()
            )));
        }
        if !adot_found {
            return Err(reader.fail(&format!(
                "ARMO record '{}' missing AODT subrecord",
                armor.id.to_debug_string()
            )));
        }

        Ok(armor)
    }
}

