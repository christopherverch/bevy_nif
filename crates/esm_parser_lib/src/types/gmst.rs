use crate::{EsmError, EsmReader, RefId, esm_record::EsmRecord, name4, types::flags::ObjectFlags};

#[derive(Debug)]
pub struct GameSetting {
    pub record_flags: ObjectFlags,
    pub id: RefId,
    pub value: (GameSettingVariant, VarType),
}
impl GameSetting {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<GameSetting, EsmError> {
        *is_deleted = false;
        let record_flags = reader.get_record_flags();
        let id = reader.read_header_refid_check_name(name4!("NAME"))?;
        let value = GameSettingVariant::read(reader, VariantFormat::Gmst)?;
        Ok(GameSetting {
            record_flags,
            id,
            value,
        })
    }
    /// Directly get the float value, converting from Int if necessary.
    /// Returns 0.0 if the variant is not a numeric type.
    pub fn as_float(&self) -> f32 {
        self.value.0.as_float()
    }

    /// Directly get the string value if applicable.
    pub fn as_str(&self) -> Option<&str> {
        self.value.0.as_str()
    }
}
impl EsmRecord for GameSetting {
    fn id(&self) -> &RefId {
        &self.id
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum GameSettingVariant {
    None,
    Int(i32),
    Float(f32),
    String(String),
}
impl GameSettingVariant {
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
        // --- PART 2: LOAD DATA
        let value = match var_type {
            VarType::None | VarType::Unknown => GameSettingVariant::None,
            VarType::String => {
                // getHString -> read_header_decode_string
                GameSettingVariant::String(reader.read_header_decode_string()?)
            }
            VarType::Int | VarType::Short | VarType::Long => {
                if format == VariantFormat::Global {
                    let raw_float: f32 = reader.read_typed_named_header(name4!("FLTV"))?;

                    // cast the f32 to i16 or i32, using specific openmw cast logic
                    match var_type {
                        VarType::Short => {
                            // Stored in i32 variant, but truncated to 16-bit range
                            GameSettingVariant::Int(openmw_float_to_i16(raw_float) as i32)
                        }
                        VarType::Long | VarType::Int => {
                            GameSettingVariant::Int(openmw_float_to_i32(raw_float))
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
                    GameSettingVariant::Int(reader.read_typed_header()?)
                }
                // VariantFormat::Local
                else {
                    match var_type {
                        VarType::Short => {
                            let value: i16 = reader.read_typed_header()?;
                            // Stored in i32 variant, but truncated to 16-bit range
                            GameSettingVariant::Int(value as i32)
                        }
                        // read i32
                        VarType::Int => GameSettingVariant::Int(reader.read_typed_header()?),
                        _ => return Err(reader.fail("unsupported local variable integer type")),
                    }
                }
            }
            VarType::Float => {
                if format == VariantFormat::Global {
                    GameSettingVariant::Float(reader.read_typed_named_header(name4!("FLTV"))?)
                } else {
                    GameSettingVariant::Float(reader.read_typed_header()?)
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
    // OpenMW casts the float to i32 first, then to the target type (i16)
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
impl GameSettingVariant {
    /// Helper to get string for magic effect names
    pub fn as_str(&self) -> Option<&str> {
        match self {
            GameSettingVariant::String(s) => Some(s.as_str()),
            _ => None,
        }
    }
    pub fn as_float(&self) -> f32 {
        match self {
            GameSettingVariant::Float(f) => *f,
            GameSettingVariant::Int(i) => *i as f32,
            _ => 0.0,
        }
    }
}
