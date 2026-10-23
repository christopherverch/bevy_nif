use crate::{
    EsmError, EsmReader, RefId,
    esm_reader::ConsumptionType,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContItem {
    pub count: i32,
    pub item: RefId,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InventoryList {
    pub list: Vec<ContItem>,
}

impl InventoryList {
    pub fn add(&mut self, reader: &mut EsmReader) -> Result<(), EsmError> {
        reader.read_sub_header()?;
        let count: i32 = reader.read_typed(ConsumptionType::SubrecordPayload)?;
        let item: RefId = reader.read_refid()?;
        self.list.push(ContItem { count, item });
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Container {
    pub record_flags: ObjectFlags,
    pub id: RefId,
    pub script: RefId,
    pub name: String,
    pub model: String,
    pub weight: f32,
    pub flags: i32,
    pub inventory: InventoryList,
}

impl EsmRecord for Container {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Cont;
}

impl Container {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut container = Container::default();
        container.record_flags = reader.get_record_flags();
        container.inventory.list.clear();
        let mut has_name = false;
        let mut has_weight = false;
        let mut has_flags = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    container.id = reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => {
                    container.model = reader.read_header_decode_string()?;
                }
                b"FNAM" => {
                    container.name = reader.read_header_decode_string()?;
                }
                b"CNDT" => {
                    container.weight = reader.read_typed(ConsumptionType::SubrecordPayload)?;
                    has_weight = true;
                }
                b"FLAG" => {
                    container.flags = reader.read_typed(ConsumptionType::SubrecordPayload)?;
                    if container.flags & 0xf4 != 0 {
                        return Err(reader.fail("Unknown flags"));
                    }
                    if container.flags & 0x8 == 0 {
                        return Err(reader.fail("Flag 8 not set"));
                    }
                    has_flags = true;
                }
                b"SCRI" => {
                    container.script = reader.read_refid()?;
                }
                b"NPCO" => {
                    container.inventory.add(reader)?;
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
        if !has_weight && !*is_deleted {
            return Err(reader.fail("Missing CNDT subrecord"));
        }
        if !has_flags && !*is_deleted {
            return Err(reader.fail("Missing FLAG subrecord"));
        }

        Ok(container)
    }

    pub fn blank(&mut self) {
        self.record_flags = ObjectFlags::empty();
        self.name.clear();
        self.model.clear();
        self.script = RefId::default();
        self.weight = 0.0;
        self.flags = 0x8; // set default flag value 
        self.inventory.list.clear();
    }
}
