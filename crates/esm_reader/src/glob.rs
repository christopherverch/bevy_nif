// src/glob.rs

use crate::esm_reader::{EsmError, EsmReader};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalVariableType {
    Short,
    Long,
    Float,
    Unknown,
}

impl Default for GlobalVariableType {
    fn default() -> Self {
        GlobalVariableType::Unknown
    }
}

impl From<u8> for GlobalVariableType {
    fn from(byte: u8) -> Self {
        match byte {
            b's' => GlobalVariableType::Short,
            b'l' => GlobalVariableType::Long,
            b'f' => GlobalVariableType::Float,
            _ => GlobalVariableType::Unknown,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Glob {
    pub id: String,
    pub var_type: GlobalVariableType,
    pub value: f32,
}

impl Glob {
    pub fn new() -> Self {
        Default::default()
    }

    /// Loads a GLOB record's data from the EsmReader.
    /// Assumes the EsmReader is positioned after the "GLOB" record name
    /// and its initial record header (size, flags) have been read.
    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut glob = Glob::new();
        let mut id_found = false;
        let mut type_found = false;
        let mut value_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;
            // println!("  [GLOB DEBUG] Subrecord: {}", sub_name.to_string_lossy());

            match &sub_name.m_data {
                b"NAME" => {
                    glob.id = reader.get_h_string()?;
                    id_found = true;
                    // println!("    [GLOB DEBUG] ID (Name): {}", glob.id);
                }
                b"FNAM" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 1 {
                        return Err(reader.report_sub_size_mismatch(1, reader.get_sub_size()));
                    }
                    let type_char: u8 = reader.get_t()?;
                    glob.var_type = GlobalVariableType::from(type_char);
                    type_found = true;
                    // println!("    [GLOB DEBUG] Type (FNAM): {:?}", glob.var_type);
                }
                b"FLTV" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    glob.value = reader.get_t::<f32>()?;
                    value_found = true;
                    // println!("    [GLOB DEBUG] Value (FLTV): {}", glob.value);
                }
                _ => {
                    // println!("    [GLOB DEBUG] Skipping unknown subrecord: {}", sub_name.to_string_lossy());
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found {
            return Err(reader.fail("GLOB record missing Name subrecord"));
        }
        if !type_found {
            return Err(reader.fail(&format!("GLOB record '{}' missing FNAM subrecord", glob.id)));
        }
        if !value_found {
            return Err(reader.fail(&format!("GLOB record '{}' missing FLTV subrecord", glob.id)));
        }

        Ok(glob)
    }
}
