// internal imports
use crate::prelude::*;

#[derive(Meta, Clone, Debug, From, PartialEq, SmartDefault)]
pub enum NiPosKey {
    #[default]
    LinKey(Vec<NiLinPosKey>),
    BezKey(Vec<NiBezPosKey>),
    TCBKey(Vec<NiTCBPosKey>),
}

#[derive(Meta, Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct NiLinPosKey {
    pub time: f32,
    pub value: Vec3,
}

#[derive(Meta, Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct NiBezPosKey {
    pub time: f32,
    pub value: Vec3,
    pub in_tan: Vec3,
    pub out_tan: Vec3,
}

#[derive(Meta, Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct NiTCBPosKey {
    pub time: f32,
    pub value: Vec3,
    pub tension: f32,
    pub continuity: f32,
    pub bias: f32,
}

impl Load for NiPosKey {
    fn load(stream: &mut Reader<'_>) -> io::Result<Self> {
        let num_keys: u32 = stream.load()?;
        let key_type = if num_keys == 0 {
            KeyType::LinKey
        } else {
            stream.load()?
        };
        Ok(match key_type {
            KeyType::LinKey => {
                let mut keys = stream.load_vec::<NiLinPosKey>(num_keys)?;
                for key in &mut keys {
                    key.value = nif_to_bevy_vec3(key.value);
                }
                NiPosKey::LinKey(keys)
            }
            KeyType::BezKey => {
                let mut keys = stream.load_vec::<NiBezPosKey>(num_keys)?;
                for key in &mut keys {
                    key.value = nif_to_bevy_vec3(key.value);
                    key.in_tan = nif_to_bevy_vec3(key.in_tan);
                    key.out_tan = nif_to_bevy_vec3(key.out_tan);
                }
                NiPosKey::BezKey(keys)
            }
            KeyType::TCBKey => {
                let mut keys = stream.load_vec::<NiTCBPosKey>(num_keys)?;
                for key in &mut keys {
                    key.value = nif_to_bevy_vec3(key.value);
                }
                NiPosKey::TCBKey(keys)
            }
            _ => Reader::error(format!("Invalid KeyType: {key_type:?}"))?,
        })
    }
}
impl Save for NiLinPosKey {
    fn save(&self, stream: &mut Writer) -> io::Result<()> {
        stream.save(&self.time)?;
        stream.save(&bevy_to_nif_vec3(self.value))?;
        Ok(())
    }
}

impl Save for NiBezPosKey {
    fn save(&self, stream: &mut Writer) -> io::Result<()> {
        stream.save(&self.time)?;
        stream.save(&bevy_to_nif_vec3(self.value))?;
        stream.save(&bevy_to_nif_vec3(self.in_tan))?;
        stream.save(&bevy_to_nif_vec3(self.out_tan))?;
        Ok(())
    }
}

impl Save for NiTCBPosKey {
    fn save(&self, stream: &mut Writer) -> io::Result<()> {
        stream.save(&self.time)?;
        stream.save(&bevy_to_nif_vec3(self.value))?;
        stream.save(&self.tension)?;
        stream.save(&self.continuity)?;
        stream.save(&self.bias)?;
        Ok(())
    }
}
impl Save for NiPosKey {
    fn save(&self, stream: &mut Writer) -> io::Result<()> {
        /// Helper function to reduce duplication
        #[inline]
        fn save<'a, I, S>(
            stream: &mut Writer,
            len: usize,
            key_type: KeyType,
            keys: I,
        ) -> io::Result<()>
        where
            I: IntoIterator<Item = &'a S>,
            S: Save + 'a,
        {
            stream.save_as::<u32>(len)?;
            if len != 0 {
                stream.save(&key_type)?;
                stream.save_seq(keys)?;
            }
            Ok(())
        }

        match self {
            NiPosKey::LinKey(keys) => save(stream, keys.len(), KeyType::LinKey, keys),
            NiPosKey::BezKey(keys) => save(stream, keys.len(), KeyType::BezKey, keys),
            NiPosKey::TCBKey(keys) => save(stream, keys.len(), KeyType::TCBKey, keys),
        }
    }
}
