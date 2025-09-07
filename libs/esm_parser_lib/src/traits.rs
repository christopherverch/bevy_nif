use glam::Vec3;

use crate::{
    RefId,
    esm_reader::{ConsumptionType, EsmError, EsmReader},
};
use std::io::{self, Write};

pub trait Load {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError>
    where
        Self: Sized;
}

pub trait Save {
    fn save(&self, stream: &mut impl Write) -> io::Result<()>;
}

impl Load for String {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut buf = vec![0u8; reader.get_sub_size() as usize];
        reader.get_exact(&mut buf, ConsumptionType::SubrecordPayload)?;
        let null_pos = buf.iter().position(|&x| x == 0);
        let string_bytes = match null_pos {
            Some(pos) => &buf[..pos],
            None => &buf,
        };

        Ok(String::from_utf8_lossy(string_bytes).into_owned())
    }
}
impl Load for RefId {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.get_ref_id()
    }
}
impl Load for u8 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.get_u8_le(ConsumptionType::SubrecordPayload)
    }
}
impl Load for [u8; 4] {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let first = reader.get_u8_le(ConsumptionType::SubrecordPayload)?;
        let second = reader.get_u8_le(ConsumptionType::SubrecordPayload)?;
        let third = reader.get_u8_le(ConsumptionType::SubrecordPayload)?;
        let fourth = reader.get_u8_le(ConsumptionType::SubrecordPayload)?;
        Ok([first, second, third, fourth])
    }
}
impl Load for u16 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.get_u16_le(ConsumptionType::SubrecordPayload)
    }
}
impl Load for u32 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.get_u32_le(ConsumptionType::SubrecordPayload)
    }
}
impl Load for f32 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.get_f32_le(ConsumptionType::SubrecordPayload)
    }
}
impl Load for i32 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.get_i32_le(ConsumptionType::SubrecordPayload)
    }
}
impl Load for Vec3 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let x = reader.get_f32_le(ConsumptionType::SubrecordPayload)?;
        let y = reader.get_f32_le(ConsumptionType::SubrecordPayload)?;
        let z = reader.get_f32_le(ConsumptionType::SubrecordPayload)?;
        Ok(Vec3::new(x, y, z))
    }
}
