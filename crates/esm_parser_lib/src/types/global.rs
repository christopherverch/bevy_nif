use crate::{
    EsmError, EsmReader, RefId,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags, variant::Variant},
};

#[derive(Clone, Debug, Default)]
pub struct Global {
    pub record_flags: ObjectFlags,
    pub id: RefId,
    pub value: Variant,
}

impl EsmRecord for Global {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Glob;
}

impl Global {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut global = Global::default();
        global.record_flags = reader.get_record_flags();

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    global.id = reader.read_refid()?;
                }
                b"FLTV" => {
                    global.value = Variant::Float(reader.read_typed_header::<f32>()?);
                }
                b"INTV" => {
                    global.value = Variant::Int(reader.read_typed_header::<i32>()?);
                }
                b"SHOR" => {
                    global.value = Variant::Short(reader.read_typed_header::<i16>()?);
                }
                b"DELE" => {
                    reader.skip_sub_header()?;
                    *is_deleted = true;
                }
                _ => {
                    return Err(reader.fail("Unknown subrecord"));
                }
            }
        }
        Ok(global)
    }

    pub fn blank(&mut self) {
        self.record_flags = ObjectFlags::empty();
        self.value = Variant::None;
    }
}
