// internal imports
use crate::prelude::*;

#[derive(Meta, Clone, Debug, PartialEq, SmartDefault)]
pub struct NiBoxBV {
    pub center: Vec3,
    #[default(Mat3::IDENTITY)]
    pub axis: Mat3,
    pub extents: Vec3,
}

impl Load for NiBoxBV {
    fn load(stream: &mut Reader<'_>) -> io::Result<Self> {
        let center = nif_to_bevy_vec3(stream.load()?);
        let axis = stream.load()?;
        let extents = nif_to_bevy_vec3(stream.load()?);
        Ok(Self {
            center,
            axis,
            extents,
        })
    }
}

impl Save for NiBoxBV {
    fn save(&self, stream: &mut Writer) -> io::Result<()> {
        stream.save(&self.center)?;
        stream.save(&self.axis)?;
        stream.save(&self.extents)?;
        Ok(())
    }
}
