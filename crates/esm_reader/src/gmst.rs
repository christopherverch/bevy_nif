// src/gmst.rs

 // Assuming Name is FixedString<4>
use crate::esm_reader::{EsmError, EsmReader};

#[derive(Debug, Clone, PartialEq)]
pub enum GmstValue {
    String(String),
    Int(i32),
    Float(f32),
    Unknown,
}

impl Default for GmstValue {
    fn default() -> Self {
        GmstValue::Unknown
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Gmst {
    pub id: String,
    pub value: GmstValue,
}

impl Gmst {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut gmst = Gmst::new();
        let mut id_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;
            // println!("  [GMST DEBUG] Subrecord: {}", sub_name.to_string_lossy());

            match &sub_name.m_data {
                // sub_name.m_data is [u8; 4]
                b"NAME" => {
                    // Literal b"Name" is treated as [b'N',b'A',b'M',b'E'] in pattern
                    gmst.id = reader.get_h_string()?;
                    id_found = true;
                    // println!("    [GMST DEBUG] ID (Name): {}", gmst.id);
                }
                b"STRV" => {
                    gmst.value = GmstValue::String(reader.get_h_string()?);
                    // if let GmstValue::String(s) = &gmst.value {
                    //     println!("    [GMST DEBUG] Value (STRV): {}", s);
                    // }
                }
                b"INTV" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    gmst.value = GmstValue::Int(reader.get_t::<i32>()?);
                    // if let GmstValue::Int(i) = &gmst.value {
                    //     println!("    [GMST DEBUG] Value (INTV): {}", i);
                    // }
                }
                b"FLTV" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    gmst.value = GmstValue::Float(reader.get_t::<f32>()?);
                    // if let GmstValue::Float(f) = &gmst.value {
                    //     println!("    [GMST DEBUG] Value (FLTV): {}", f);
                    // }
                }
                _ => {
                    // println!("    [GMST DEBUG] Skipping unknown subrecord: {}", sub_name.to_string_lossy());
                    reader.skip_h_sub()?; // This method needs to exist on EsmReader
                }
            }
        }

        if !id_found && gmst.id.is_empty() {
            return Err(reader.fail("GMST record missing Name subrecord"));
        }

        Ok(gmst)
    }
}
