use crate::{EsmError, EsmReader, RefId};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct InventoryList {
    pub list: Vec<ContItem>,
}
impl InventoryList {
    pub fn add(&mut self, reader: &mut EsmReader) -> Result<(), EsmError> {
        reader.read_sub_header()?;
        let count: i32 = reader.load()?;
        let item = reader.get_maybe_fixed_refid_sized(32)?;
        self.list.push(ContItem { count, item });
        Ok(())
    }
}
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ContItem {
    count: i32,
    item: RefId,
}
