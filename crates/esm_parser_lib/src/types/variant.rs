use crate::{EsmError, EsmReader, name4};

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Variant {
    #[default]
    None,
    Int(i32),
    Short(i16),
    Float(f32),
    Long(i32),
    String(String),
}
impl Variant {
    pub fn read(
        reader: &mut EsmReader,
        format: VariantFormat,
    ) -> Result<(Self, VarType), EsmError> {
        let var_type;
        match format {
            VariantFormat::Global => {
                let type_id = reader.read_header_decode_string_validate_subname(name4!("FNAM"))?;
                var_type = match type_id.as_str() {
                    "s" => VarType::Short,
                    "l" => VarType::Long,
                    "f" => VarType::Float,
                    _ => {
                        return Err(
                            reader.fail(&format!("illegal global variable type {}", type_id))
                        );
                    }
                };
            }
            VariantFormat::Gmst => {
                if !reader.has_more_subs() {
                    var_type = VarType::None;
                } else {
                    let sub_name = reader.get_or_read_sub_name()?;
                    var_type = match sub_name.as_bytes() {
                        b"STRV" => VarType::String,
                        b"INTV" => VarType::Int,
                        b"FLTV" => VarType::Float,
                        _ => return Err(reader.fail(&format!("invalid subrecord: {:?}", sub_name))),
                    };
                }
            }
            VariantFormat::Info => {
                let sub_name = reader.get_or_read_sub_name()?;
                var_type = match sub_name.as_bytes() {
                    b"INTV" => VarType::Int,
                    b"FLTV" => VarType::Float,
                    _ => return Err(reader.fail(&format!("invalid subrecord: {:?}", sub_name))),
                };
            }
            VariantFormat::Local => {
                let sub_name = reader.get_or_read_sub_name()?;
                var_type = match sub_name.as_bytes() {
                    b"INTV" => VarType::Int,
                    b"FLTV" => VarType::Float,
                    b"STTV" if format == VariantFormat::Local => VarType::Short,
                    _ => return Err(reader.fail(&format!("invalid subrecord: {:?}", sub_name))),
                };
            }
        }
        // Load data
        let value = match var_type {
            VarType::None | VarType::Unknown => Variant::None,
            VarType::String => Variant::String(reader.read_header_decode_string()?),
            VarType::Int | VarType::Short | VarType::Long => {
                if format == VariantFormat::Global {
                    let raw_float: f32 = reader.read_typed_named_header(name4!("FLTV"))?;

                    // cast the f32 to i16 or i32
                    match var_type {
                        VarType::Short => {
                            // Stored in i32 variant, but truncated to 16-bit range
                            Variant::Int(openmw_float_to_i16(raw_float) as i32)
                        }
                        VarType::Long | VarType::Int => {
                            Variant::Int(openmw_float_to_i32(raw_float))
                        }
                        _ => unreachable!(),
                    }
                } else if format == VariantFormat::Gmst || format == VariantFormat::Info {
                    if var_type != VarType::Int {
                        let error_type = if format == VariantFormat::Gmst {
                            "gmst"
                        } else {
                            "info"
                        };
                        return Err(reader
                            .fail(&format!("unsupported {} variable integer type", error_type)));
                    }
                    // GMSTs/Locals usually have the proper subrecord type
                    Variant::Int(reader.read_typed_header()?)
                } else {
                    match var_type {
                        VarType::Short => {
                            let value: i16 = reader.read_typed_header()?;
                            // Stored in i32 variant, but truncated to 16-bit range
                            Variant::Int(value as i32)
                        }
                        // read i32
                        VarType::Int => Variant::Int(reader.read_typed_header()?),
                        _ => return Err(reader.fail("unsupported local variable integer type")),
                    }
                }
            }
            VarType::Float => {
                if format == VariantFormat::Global {
                    Variant::Float(reader.read_typed_named_header(name4!("FLTV"))?)
                } else {
                    Variant::Float(reader.read_typed_header()?)
                }
            }
        };

        Ok((value, var_type))
    }
}
fn openmw_float_to_i32(value: f32) -> i32 {
    let min = i32::MIN as f64;
    let max = i32::MAX as f64;
    let v = value as f64;

    if value.is_nan() || v < min || v > max {
        i32::MIN
    } else {
        value as i32
    }
}

fn openmw_float_to_i16(value: f32) -> i16 {
    openmw_float_to_i32(value) as i16
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VariantFormat {
    Global,
    Gmst,
    Info,
    Local,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VarType {
    Unknown,
    None,
    Short,
    Int,
    Long,
    Float,
    String,
}
impl Variant {
    pub fn as_float(&self) -> f32 {
        match self {
            Variant::Float(f) => *f,
            Variant::Int(i) => *i as f32,
            Variant::Short(s) => *s as f32,
            Variant::Long(l) => *l as f32,
            _ => 0.0,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Variant::String(s) => Some(s.as_str()),
            _ => None,
        }
    }
}
