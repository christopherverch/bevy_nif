use glam::Vec3;

use crate::{EsmError, EsmReader, Load};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TravelDestination {
    pub translation: Vec3,
    pub rotation: Vec3,
    pub cell: String,
}
impl Load for TravelDestination {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let translation = reader.load()?;
        let rotation = reader.load()?;
        // The 'cell' field is a separate subrecord and should be
        // handled by the parent 'Reference' record's parser.
        // We initialize it here and it will be filled in by the parent.
        Ok(Self {
            translation,
            rotation,
            cell: String::new(),
        })
    }
}
