use bitflags::bitflags;

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
