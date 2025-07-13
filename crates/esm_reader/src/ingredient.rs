// src/ingredient.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct IngredientData {
    pub weight: f32,
    pub value: i32,
    pub effect_id: [i32; 4],    // IDs of the 4 magic effects
    pub skill_id: [i32; 4],     // Skill affected by each effect (-1 for none)
    pub attribute_id: [i32; 4], // Attribute affected by each effect (-1 for none)
} // Total: 4 + 4 + (4*4) + (4*4) + (4*4) = 8 + 16 + 16 + 16 = 56 bytes.
// This matches OpenMW's ESM::Ingredient::mData (IRDTstruct).

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ingredient {
    pub id: RefId,                    // From NAME subrecord
    pub model: Option<String>,        // From MODL subrecord
    pub full_name: Option<String>,    // From FNAM subrecord
    pub data: Option<IngredientData>, // From IRDT subrecord (or IGDT in some docs)
    // OpenMW uses "IRDT".
    pub icon: Option<String>,     // From ITEX subrecord
    pub script_id: Option<RefId>, // From SCRI subrecord (optional)
}

impl Ingredient {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut ingr = Ingredient::new();
        let mut id_found = false;
        let mut irdt_found = false;
        let mut fnam_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    ingr.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    ingr.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    ingr.full_name = Some(reader.get_h_string()?);
                    fnam_found = true;
                }
                b"IRDT" => {
                    // Ingredient Data
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<IngredientData>() as u32; // 56 bytes
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }

                    let mut data_val = IngredientData::default();
                    data_val.weight = reader.get_t::<f32>()?;
                    data_val.value = reader.get_t::<i32>()?;
                    for i in 0..4 {
                        data_val.effect_id[i] = reader.get_t::<i32>()?;
                    }
                    for i in 0..4 {
                        data_val.skill_id[i] = reader.get_t::<i32>()?;
                    }
                    for i in 0..4 {
                        data_val.attribute_id[i] = reader.get_t::<i32>()?;
                    }
                    ingr.data = Some(data_val);
                    irdt_found = true;
                }
                b"ITEX" => {
                    ingr.icon = Some(reader.get_h_string()?);
                }
                b"SCRI" => {
                    ingr.script_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || ingr.id.empty() {
            return Err(reader.fail("INGR record missing mandatory NAME subrecord for its ID"));
        }
        if !fnam_found {
            return Err(reader.fail(&format!(
                "INGR record '{}' missing FNAM subrecord",
                ingr.id.to_debug_string()
            )));
        }
        if !irdt_found {
            return Err(reader.fail(&format!(
                "INGR record '{}' missing IRDT subrecord",
                ingr.id.to_debug_string()
            )));
        }

        Ok(ingr)
    }
}

