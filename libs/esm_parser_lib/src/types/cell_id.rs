use crate::{
    EsmError, EsmReader, Load, Name32, RefId, esm_reader::ConsumptionType, refid_types::INTERNER,
};
#[derive(PartialEq)]
pub struct CellId {
    pub world_space: String,
    pub index: CellIndex,
    pub paged: bool,
}
#[derive(PartialEq, Default)]
pub struct CellIndex {
    pub x: i32,
    pub y: i32,
}
impl Load for CellIndex {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let x = reader.read_typed::<i32>(ConsumptionType::SubrecordPayload)?;
        let y = reader.read_typed::<i32>(ConsumptionType::SubrecordPayload)?;
        Ok(Self { x, y })
    }
}
impl Load for CellId {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut index = CellIndex::default();
        let world_space = reader
            .read_header_string_with_name(Name32::new("SPAC"))?
            .to_string();
        let mut paged = false;
        if reader.is_next_sub(Name32::new("CIDX"))? {
            // Get the subrecord header and read the payload using the Load trait for CellIndex
            reader.read_sub_header()?;
            index = reader.load()?;
            paged = true;
        }

        Ok(Self {
            world_space,
            index,
            paged,
        })
    }
}
impl CellId {
    pub fn extract_from_ref_id(id: &RefId) -> Result<Self, EsmError> {
        match id {
            RefId::Empty(_) => Ok(Self {
                world_space: String::new(),
                index: CellIndex::default(),
                paged: true,
            }),
            RefId::SizedString(string_ref_id) | RefId::UnsizedString(string_ref_id) => {
                let interner_guard = INTERNER.lock().unwrap();
                let contained_string = interner_guard.resolve(string_ref_id.0);
                match contained_string {
                    Some(s) => Ok(Self {
                        world_space: s.to_string(),
                        index: CellIndex::default(),
                        paged: false,
                    }),
                    None => Err(EsmError::Custom("Interned string not found".to_string())),
                }
            }
            RefId::ESM3ExteriorCell(exterior_cell_ref_id) => Ok(Self {
                world_space: String::new(),
                index: CellIndex {
                    x: exterior_cell_ref_id.x,
                    y: exterior_cell_ref_id.y,
                },
                paged: true,
            }),
            _ => Err(EsmError::Custom(
                "cannot extract CellId from this RefId type".to_string(),
            )),
        }
    }
}
