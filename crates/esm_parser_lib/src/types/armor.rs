use crate::{
    EsmError, EsmReader, Load, Name4, RefId,
    esm_record::EsmRecord,
    types::{
        enums::{BipedObjectType, RecInt},
        flags::ObjectFlags,
    },
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Armor {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub script: RefId,
    pub enchant: RefId,
    pub name: String,
    pub model: String,
    pub icon: String,
    pub data: ArmorData,
    pub parts: Vec<PartReference>,
}

impl EsmRecord for Armor {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Armo;
}

impl Armor {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut armor = Armor::default();
        armor.flags = reader.get_record_flags();

        let mut has_name = false;
        let mut has_data = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    armor.id = reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => {
                    let model = reader.read_header_decode_string()?;
                    armor.model = model.replace('\\', "/");
                }
                b"FNAM" => {
                    armor.name = reader.read_header_decode_string()?;
                }
                b"AODT" => {
                    reader.read_sub_header()?;
                    armor.data = reader.load()?;
                    has_data = true;
                }
                b"SCRI" => {
                    armor.script = reader.read_refid()?;
                }
                b"ITEX" => {
                    let icon = reader.read_header_decode_string()?;
                    armor.icon = icon.replace('\\', "/");
                }
                b"ENAM" => {
                    armor.enchant = reader.read_refid()?;
                }
                b"INDX" => {
                    let part = PartReference::load(reader)?;
                    armor.parts.push(part);
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

        if !has_name {
            return Err(reader.fail("Missing NAME subrecord"));
        }
        if !has_data && !*is_deleted {
            return Err(reader.fail("Missing AODT subrecord"));
        }

        Ok(armor)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ArmorData {
    pub armor_type: i32,
    pub weight: f32,
    pub value: i32,
    pub health: i32,
    pub enchant: i32,
    pub armor: i32,
}

unsafe impl bytemuck::Pod for ArmorData {}
unsafe impl bytemuck::Zeroable for ArmorData {}

impl Load for ArmorData {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let data: ArmorData =
            reader.read_typed(crate::esm_reader::ConsumptionType::SubrecordPayload)?;
        Ok(data)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PartReference {
    pub part: BipedObjectType,
    pub male: RefId,
    pub female: RefId,
}

impl PartReference {
    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.read_sub_header()?;
        let part_val: u8 =
            reader.read_typed(crate::esm_reader::ConsumptionType::SubrecordPayload)?;

        let male = reader.read_refid_named_sub_or_default(Name4::new("BNAM"))?;
        let female = reader.read_refid_named_sub_or_default(Name4::new("CNAM"))?;

        Ok(Self {
            part: BipedObjectType::from_u8_unchecked(part_val),
            male,
            female,
        })
    }
}
