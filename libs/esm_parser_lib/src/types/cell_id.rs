use crate::{EsmError, EsmReader, Load};
#[derive(PartialEq)]
struct CellId {
    world_space: String,
    index: CellIndex,
    paged: bool,
}
#[derive(PartialEq)]
struct CellIndex {
    x: i32,
    y: i32,
}
impl Load for CellId {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {}
}
