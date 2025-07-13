// src/filter.rs

use crate::common_types::Name; // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId; // ID is a RefId

// Represents one filter entry (FIDX / DATA pair)
// OpenMW ESM::Filter::FDATA struct
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct FilterConditionData {
    pub filter_type: u8, // See FilterType enum below
    pub function: u8,    // Function index (e.g., GetPCSkill, GetFactionRank)
    pub comparison: u8,  // Comparison type (e.g., ==, !=, >, <)
    pub _padding: u8,
    pub id_function_param: Name, // 4-char ID (e.g., skill ID, faction ID, global var ID)
    pub value_i: i32,            // Integer value for comparison
    pub value_f: f32,            // Float value for comparison
} // Total: 1+1+1+1 + 4 + 4 + 4 = 16 bytes

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    Function = 0,
    Global = 1,
    Local = 2, // Script local variable
    Journal = 3,
    Item = 4,
    Dead = 5,
    NotId = 6, // Actor ID
    NotFaction = 7,
    NotClass = 8,
    NotRace = 9,
    NotCell = 10,
    NotLocal = 11,
    Unknown(u8),
}
impl From<u8> for FilterType {
    /* ... conversion ... */
    fn from(val: u8) -> Self {
        match val {
            0 => FilterType::Function,
            1 => FilterType::Global,
            2 => FilterType::Local,
            3 => FilterType::Journal,
            4 => FilterType::Item,
            5 => FilterType::Dead,
            6 => FilterType::NotId,
            7 => FilterType::NotFaction,
            8 => FilterType::NotClass,
            9 => FilterType::NotRace,
            10 => FilterType::NotCell,
            11 => FilterType::NotLocal,
            _ => FilterType::Unknown(val),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filter {
    pub id: RefId,                            // From NAME subrecord
    pub conditions: Vec<FilterConditionData>, // From FIDX/DATA pairs
}

impl Filter {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut filt = Filter::new();
        let mut id_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    filt.id = reader.get_ref_id()?;
                    id_found = true;
                }
                // OpenMW loads FIDX and DATA subrecords for each condition.
                // FIDX seems to be the filter type (byte), then DATA is the rest of FilterConditionData.
                // UESP says: FIDX (byte, filter type), then FVAL (float), IVAL (long), SVAL (string[32])
                // This is different from OpenMW's Filter::FDATA struct.
                // Let's assume "FDATA" subrecord contains the FilterConditionData struct.
                // OpenMW's `loadfilt.cpp` reads "FIDX" (1 byte type), then "DATA" (15 bytes).
                // The 15 bytes are: func(u8), comp(u8), pad(u8), id(Name), val_i(i32), val_f(f32).
                // This sums to 1+1+1+4+4+4 = 15 bytes. Total 16 with FIDX.
                // My FilterConditionData struct is 16 bytes already.
                // So, each condition is likely one "FIDX" subrecord followed by one "DATA" subrecord
                // that contains the rest of the data.
                // This is complex. Let's simplify by assuming a single subrecord type holds all condition data.
                // OpenMW's `ESM::Filter::load` reads "FIDX" then "DATA" for each condition.
                // This means we need to pair them.
                // For now, let's assume a subrecord "COND" that holds FilterConditionData.
                // This is a simplification. A more accurate parser would handle FIDX then DATA.
                //
                // Re-evaluating: OpenMW's `DialogueCondition::load` (used by INFO) reads SCVR which is a list.
                // `ESM::Filter::load` reads FIDX then DATA.
                // Let's try to parse FIDX and then DATA.
                b"FIDX" => {
                    // Filter Index / Type
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 1 {
                        return Err(reader.report_sub_size_mismatch(1, reader.get_sub_size()));
                    }
                    let filter_type_byte = reader.get_t::<u8>()?;

                    // Expect DATA subrecord next for this condition
                    if reader.is_next_sub(Name::new("DATA"))? {
                        reader.get_sub_header()?;
                        // DATA for filter condition is: func(u8),comp(u8),pad(u8),id(Name),val_i(i32),val_f(f32) = 15 bytes
                        let expected_data_size = 1 + 1 + 1 + 4 + 4 + 4; // 15 bytes
                        if reader.get_sub_size() != expected_data_size {
                            return Err(reader.report_sub_size_mismatch(
                                expected_data_size,
                                reader.get_sub_size(),
                            ));
                        }
                        let mut cond_data = FilterConditionData::default();
                        cond_data.filter_type = filter_type_byte;
                        cond_data.function = reader.get_t::<u8>()?;
                        cond_data.comparison = reader.get_t::<u8>()?;
                        cond_data._padding = reader.get_t::<u8>()?;
                        cond_data.id_function_param = reader.get_t::<Name>()?;
                        cond_data.value_i = reader.get_t::<i32>()?;
                        cond_data.value_f = reader.get_t::<f32>()?;
                        filt.conditions.push(cond_data);
                    } else {
                        return Err(reader.fail(&format!(
                            "FILT record '{}': Expected DATA subrecord after FIDX",
                            filt.id.to_debug_string()
                        )));
                    }
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || filt.id.empty() {
            return Err(reader.fail("FILT record missing mandatory NAME subrecord for its ID"));
        }
        // A filter should generally have conditions.
        if filt.conditions.is_empty() {
            return Err(reader.fail(&format!(
                "FILT record '{}' has no conditions (FIDX/DATA pairs)",
                filt.id.to_debug_string()
            )));
        }

        Ok(filt)
    }
}

