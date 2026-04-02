use std::io::{self, Write};

use bitflags::bitflags;
use bytemuck::Pod;

use crate::{EsmError, EsmReader, Load, Save, esm_reader::ConsumptionType};

bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct ObjectFlags: u32 {
        const MODIFIED = 0x2;
        const DELETED = 0x20;
        const PERSISTENT = 0x400;
        const IGNORED = 0x1000;
        const BLOCKED = 0x2000;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
    pub struct CellFlags: u32 {
        const IS_INTERIOR = 0x01;
        const HAS_WATER = 0x02;
        const ILLEGAL_TO_REST = 0x04;
        const QUASI_EXTERIOR = 0x08;
    }
}
impl Load for ObjectFlags {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        ObjectFlags::from_bits(reader.read_typed(ConsumptionType::SubrecordPayload)?).ok_or_else(
            || EsmError::Custom("Failed to convert flag bytes to ObjectFlags!".to_string()),
        )
    }
}
bitflags! {
#[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct RaceFlags: i32 {
        const PLAYABLE = 0x01;
        const BEAST    = 0x02;
    }
}
impl Load for RaceFlags {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let raw: i32 = Load::load(reader)?; // read the raw i32
        Ok(RaceFlags::from_bits_truncate(raw)) // convert into bitflags
    }
}
