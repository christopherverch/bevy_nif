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
        reader.read_exact(&mut buf, ConsumptionType::SubrecordPayload)?;
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
        reader.read_ref_id()
    }
}
impl Load for u8 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.read_typed(ConsumptionType::SubrecordPayload)
    }
}
impl Load for [u8; 4] {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let first = reader.read_typed(ConsumptionType::SubrecordPayload)?;
        let second = reader.read_typed(ConsumptionType::SubrecordPayload)?;
        let third = reader.read_typed(ConsumptionType::SubrecordPayload)?;
        let fourth = reader.read_typed(ConsumptionType::SubrecordPayload)?;
        Ok([first, second, third, fourth])
    }
}
impl Load for i8 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.read_typed(ConsumptionType::SubrecordPayload)
    }
}
impl Load for u16 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.read_typed(ConsumptionType::SubrecordPayload)
    }
}
impl Load for u32 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.read_typed(ConsumptionType::SubrecordPayload)
    }
}
impl Load for f32 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.read_typed(ConsumptionType::SubrecordPayload)
    }
}
impl Load for i32 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.read_typed(ConsumptionType::SubrecordPayload)
    }
}
impl Load for Vec3 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let x = reader.read_typed(ConsumptionType::SubrecordPayload)?;
        let y = reader.read_typed(ConsumptionType::SubrecordPayload)?;
        let z = reader.read_typed(ConsumptionType::SubrecordPayload)?;
        Ok(Vec3::new(x, y, z))
    }
}
