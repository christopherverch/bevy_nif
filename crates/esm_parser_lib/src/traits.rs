use glam::Vec3;

use crate::{
    RefId,
    common_types::Name32,
    esm_reader::{ConsumptionType, EsmError, EsmReader},
};
use std::{
    io::{self},
    mem::MaybeUninit,
};

pub trait Load {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError>
    where
        Self: Sized;
}

pub trait Save {
    fn save(&self, stream: &mut impl ::std::io::Write) -> io::Result<()>;
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
        reader.read_refid()
    }
}
impl Load for u8 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.read_typed(ConsumptionType::SubrecordPayload)
    }
}

// TODO:: Slower than manual impls for each potential array size, but whatever
impl<L, const N: usize> Load for [L; N]
where
    L: Load,
{
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        // Use MaybeUninit to safely initialize an array element by element
        let mut array: [MaybeUninit<L>; N] = unsafe { MaybeUninit::uninit().assume_init() };

        for i in 0..N {
            array[i] = MaybeUninit::new(L::load(reader)?);
        }

        // Safety: all elements are now initialized
        let array = unsafe { std::mem::transmute_copy::<_, [L; N]>(&array) };
        Ok(array)
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
impl Load for i16 {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.read_typed(ConsumptionType::SubrecordPayload)
    }
}
impl Load for Name32 {
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
