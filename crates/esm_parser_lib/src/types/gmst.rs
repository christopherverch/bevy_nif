use crate::{
    EsmError, EsmReader, RefId,
    esm_record::EsmRecord,
    name4,
    types::{
        enums::RecInt,
        flags::ObjectFlags,
        variant::{VarType, Variant, VariantFormat},
    },
};

#[derive(Debug)]
pub struct GameSetting {
    pub record_flags: ObjectFlags,
    pub id: RefId,
    pub value: (Variant, VarType),
}
impl GameSetting {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<GameSetting, EsmError> {
        *is_deleted = false;
        let record_flags = reader.get_record_flags();
        let id = reader.read_header_refid_check_name(name4!("NAME"))?;
        let value = Variant::read(reader, VariantFormat::Gmst)?;
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
    const RECORD_ID: RecInt = RecInt::Gmst;
}
