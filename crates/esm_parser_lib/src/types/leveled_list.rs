use crate::{
    EsmError, EsmReader, RefId,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LevelledListBase {
    pub record_flags: ObjectFlags,
    pub flags: u32,
    pub id: RefId,
    pub chance_none: u8,
    pub list: Vec<LevelItem>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LevelItem {
    pub id: RefId,
    pub level: u16,
}

impl EsmRecord for LevelledListBase {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Levi;
}

impl LevelledListBase {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut levelled_list = LevelledListBase::default();
        levelled_list.record_flags = reader.get_record_flags();

        let mut has_name = false;
        let mut has_list = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    levelled_list.id = reader.read_refid()?;
                    has_name = true;
                }
                b"DATA" => {
                    levelled_list.flags = reader.read_typed_header()?;
                }
                b"NNAM" => {
                    levelled_list.chance_none = reader.load()?;
                }
                b"INDX" => {
                    let length: u32 = reader.load()?;
                    levelled_list
                        .list
                        .resize(length as usize, LevelItem::default());

                    for item in &mut levelled_list.list {
                        item.id = reader.read_refid()?;
                        item.level = reader.load()?;
                    }

                    has_list = true;
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
        if !has_list && !*is_deleted {
            return Err(reader.fail("Missing INDX subrecord"));
        }

        Ok(levelled_list)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CreatureLevList {
    pub base: LevelledListBase,
}

impl EsmRecord for CreatureLevList {
    fn id(&self) -> &RefId {
        self.base.id()
    }
    const RECORD_ID: RecInt = RecInt::Crec;
}

impl CreatureLevList {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        let base = LevelledListBase::load(reader, is_deleted)?;
        Ok(CreatureLevList { base })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemLevList {
    pub base: LevelledListBase,
}

impl EsmRecord for ItemLevList {
    fn id(&self) -> &RefId {
        self.base.id()
    }
    const RECORD_ID: RecInt = RecInt::Levi;
}

impl ItemLevList {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        let base = LevelledListBase::load(reader, is_deleted)?;
        Ok(ItemLevList { base })
    }
}
