use crate::{EsmError, EsmReader, Load, Name4, RefId, esm_reader::ConsumptionType};
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
            .read_header_string_with_name(Name4::new("SPAC"))?
            .to_string();
        let mut paged = false;
        if reader.is_next_sub(Name4::new("CIDX"))? {
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
    pub fn extract_from_ref_id(id: &RefId) -> Self {
        match id {
            RefId::Empty(_) => Self {
                paged: true,
                index: CellIndex::default(),
                world_space: String::new(),
            },

            RefId::SizedString(s) | RefId::UnsizedString(s) => Self {
                paged: false,
                world_space: s.0.to_string(), // Your interned &'static str
                index: CellIndex { x: 0, y: 0 },
            },

            RefId::ESM3ExteriorCell(cell_ref) => Self {
                paged: true,
                index: CellIndex {
                    x: cell_ref.x,
                    y: cell_ref.y,
                },
                world_space: String::new(),
            },

            // template <typename T> ... throw std::runtime_error
            _ => panic!("cannot extract CellId from this Id type: {:?}", id),
        }
    }
}
