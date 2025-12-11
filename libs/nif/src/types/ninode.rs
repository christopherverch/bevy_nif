// internal imports
use crate::prelude::*;

#[derive(Meta, Clone, Debug, Default, PartialEq)]
pub struct NiNode {
    pub base: NiAVObject,
    pub children: Vec<NiLink<NiAVObject>>,
    pub effects: Vec<NiLink<NiDynamicEffect>>,
}

impl Load for NiNode {
    fn load(stream: &mut Reader<'_>) -> io::Result<Self> {
        let base = stream.load()?;
        let children = stream.load()?;
        let effects = stream.load()?;
        Ok(Self {
            base,
            children,
            effects,
        })
    }
}

impl Save for NiNode {
    fn save(&self, stream: &mut Writer) -> io::Result<()> {
        stream.save(&self.base)?;
        stream.save(&self.children)?;
        stream.save(&self.effects)?;
        Ok(())
    }
}
