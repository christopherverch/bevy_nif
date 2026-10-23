use crate::types::enums::{FilterComparison, FilterFunction, FilterType};
use crate::types::variant::{VarType, Variant, VariantFormat};
use crate::{EsmError, EsmReader, RefId}; // Bring in Variant and VarType

#[derive(Clone, Debug, PartialEq)]
pub struct DialogueCondition {
    pub index: u8,
    pub filter_type: FilterType,
    pub filter_function: FilterFunction,
    pub comparison: FilterComparison,
    pub id: String,
    pub value: Variant, // Replaced DialogueValue with your shared Variant type
}

impl DialogueCondition {
    pub fn load(reader: &mut EsmReader, context: &RefId) -> Result<Option<Self>, EsmError> {
        let rule = reader.read_header_decode_string()?;

        // 1. Read the Variant value using VariantFormat::Info (matching OpenMW)
        // SCVR records in INFO format always expect a subsequent value subrecord.
        let (variant_value, var_type) = Variant::read(reader, VariantFormat::Info)?;

        if rule.len() < 5 {
            eprintln!(
                "Found invalid SCVR rule of size {} in INFO {}",
                rule.len(),
                context
            );
            return Ok(None);
        }

        let bytes = rule.as_bytes();

        // 2. Validate comparison operator at index 4 ('0' to '5')
        let comp_byte = bytes[4];
        if !(b'0'..=b'5').contains(&comp_byte) {
            eprintln!(
                "Found invalid SCVR comparison operator {} in INFO {}",
                comp_byte as char, context
            );
            return Ok(None);
        }
        // Safety: ensure your FilterComparison layout matches '0'..'5' byte values
        let comparison = unsafe { std::mem::transmute::<u8, FilterComparison>(comp_byte) };

        // 3. Parse index at index 0
        let index = if (b'0'..=b'9').contains(&bytes[0]) {
            bytes[0] - b'0'
        } else {
            eprintln!("Found invalid SCVR index {} in INFO {}", bytes[0], context);
            0
        };

        let filter_type;
        let filter_function = FilterFunction::default();

        if bytes[1] == b'1' {
            if rule.len() < 4 {
                eprintln!("Encountered short SCVR function rule in INFO {}", context);
                return Ok(None);
            }
            filter_type = FilterType::Function;
            let func_chunk = u16::from_le_bytes([bytes[2], bytes[3]]);

            // (Keep your existing match block for filter_function here...)
            let _ = func_chunk; // placeholder for your long function match block
        } else if (b'2'..=b'9').contains(&bytes[1]) || (b'A'..=b'C').contains(&bytes[1]) {
            if rule.len() == 5 {
                eprintln!(
                    "Missing id for SCVR of type {} in INFO {}",
                    bytes[1] as char, context
                );
                return Ok(None);
            }

            let mut malformed = bytes[3] != b'X';
            filter_type = match bytes[1] {
                b'2' => {
                    malformed |= bytes[2] != b'f' && bytes[2] != b'l' && bytes[2] != b's';
                    FilterType::Global
                }
                b'3' => {
                    malformed |= bytes[2] != b'f' && bytes[2] != b'l' && bytes[2] != b's';
                    FilterType::Local
                }
                b'4' => {
                    malformed |= bytes[2] != b'J';
                    FilterType::Journal
                }
                b'5' => {
                    malformed |= bytes[2] != b'I';
                    FilterType::Item
                }
                b'6' => {
                    malformed |= bytes[2] != b'D';
                    FilterType::Dead
                }
                b'7' => {
                    malformed |= bytes[2] != b'X';
                    FilterType::NotId
                }
                b'8' => {
                    malformed |= bytes[2] != b'F';
                    FilterType::NotFaction
                }
                b'9' => {
                    malformed |= bytes[2] != b'C';
                    FilterType::NotClass
                }
                b'A' => {
                    malformed |= bytes[2] != b'R';
                    FilterType::NotRace
                }
                b'B' => {
                    malformed |= bytes[2] != b'L';
                    FilterType::NotCell
                }
                b'C' => {
                    malformed |= bytes[2] != b'f' && bytes[2] != b'l' && bytes[2] != b's';
                    FilterType::NotLocal
                }
                _ => FilterType::None,
            };

            if malformed {
                eprintln!("Found malformed SCVR rule in INFO {}", context);
            }
        } else {
            eprintln!(
                "Found invalid SCVR function {} in INFO {}",
                bytes[1] as char, context
            );
            return Ok(None);
        }

        let id = rule[5..].to_string();

        // 4. Validate variant type matching expectations (equivalent to C++ checking VT_Int/VT_Float)
        match var_type {
            VarType::Int | VarType::Float | VarType::Short | VarType::Long => {}
            other => {
                eprintln!(
                    "Found invalid SCVR variant type {:?} in INFO {}",
                    other, context
                );
                return Ok(None);
            }
        }

        Ok(Some(DialogueCondition {
            index,
            filter_type,
            filter_function,
            comparison,
            id,
            value: variant_value,
        }))
    }
}
