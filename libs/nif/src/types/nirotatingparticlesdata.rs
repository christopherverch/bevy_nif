// internal imports
use crate::prelude::*;

#[derive(Meta, Clone, Debug, PartialEq, SmartDefault)]
pub struct NiRotatingParticlesData {
    pub base: NiParticlesData,
    pub rotations: Vec<Quat>,
}

impl Load for NiRotatingParticlesData {
    fn load(stream: &mut Reader<'_>) -> io::Result<Self> {
        let base: NiParticlesData = stream.load()?;
        let has_rotations = stream.load::<u32>()? != 0;
        let num_rotations = if has_rotations {
            base.vertices.len()
        } else {
            0
        };
        let mut rotations: Vec<Quat> = stream.load_seq(num_rotations)?;
        for rotation in rotations.iter_mut() {
            *rotation = nif_to_bevy_quat(*rotation);
        }
        Ok(Self { base, rotations })
    }
}

impl Save for NiRotatingParticlesData {
    fn save(&self, stream: &mut Writer) -> io::Result<()> {
        stream.save(&self.base)?;

        let has_rotations = !self.rotations.is_empty();
        stream.save_as::<u32>(has_rotations)?;

        if has_rotations {
            // Convert each rotation back to NIF space before writing
            let nif_rotations: Vec<Quat> = self
                .rotations
                .iter()
                .map(|&rot| nif_to_bevy_quat(rot))
                .collect();
            stream.save_seq(&nif_rotations)?;
        }

        Ok(())
    }
}
