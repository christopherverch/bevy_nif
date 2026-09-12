use crate::{
    EsmError, EsmReader, Load, RefId,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Bodypart {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub race: RefId,
    pub model: String,
    pub data: BodypartData,
}
impl EsmRecord for Bodypart {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Body;
}
impl Bodypart {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut body_part = Bodypart::default();
        body_part.flags = reader.get_record_flags();
        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    body_part.id = reader.read_refid()?;
                }
                b"MODL" => {
                    let model = reader.read_header_decode_string()?;
                    let normalized = model.replace('\\', "/");
                    body_part.model = normalized;
                }
                b"FNAM" => {
                    body_part.race = reader.read_refid()?;
                }
                b"BYDT" => {
                    reader.read_sub_header()?;
                    body_part.data = reader.load()?;
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
        Ok(body_part)
    }
    pub fn is_first_person_bodypart(&self) -> bool {
        return self.id.ends_with("1st");
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BodypartData {
    pub part: BodypartId,
    pub vampire: bool,
    pub flags: BodypartFlags,
    pub bodypart_type: BodypartType,
}
impl Load for BodypartData {
    fn load(reader: &mut EsmReader) -> Result<BodypartData, EsmError> {
        let [part_raw, vampire_raw, flags_raw, type_raw] = reader.load()?;

        Ok(Self {
            part: BodypartId::from_u8_unchecked(part_raw),
            vampire: vampire_raw != 0,
            flags: BodypartFlags::from_bits_truncate(flags_raw),
            bodypart_type: BodypartType::from_u8_unchecked(type_raw),
        })
    }
}
#[repr(u8)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodypartId {
    #[default]
    Head = 0,
    Hair = 1,
    Neck = 2,
    Chest = 3,
    Groin = 4,
    Hand = 5,
    Wrist = 6,
    Forearm = 7,
    Upperarm = 8,
    Foot = 9,
    Ankle = 10,
    Knee = 11,
    Upperleg = 12,
    Clavicle = 13,
    Tail = 14,
}
impl BodypartId {
    #[inline]
    pub fn from_u8_unchecked(value: u8) -> Self {
        debug_assert!(value <= 14);
        unsafe { std::mem::transmute(value) }
    }
}

bitflags::bitflags! {
#[derive(Clone, Debug, Default, Eq, PartialEq)]
    pub struct BodypartFlags: u8 {
        const FEMALE = 0x01;
        const NOT_PLAYABLE = 0x02;
    }
}
#[repr(u8)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BodypartType {
    #[default]
    Skin = 0,
    Clothing = 1,
    Armor = 2,
}
impl BodypartType {
    pub fn from_u8_unchecked(value: u8) -> Self {
        debug_assert!(value <= 2);
        unsafe { std::mem::transmute(value) }
    }
}
