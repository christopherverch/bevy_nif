// src/common_types.rs

use std::path::PathBuf;
use std::str;

// --- From esmcommon.hpp ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RecordFlag {
    Deleted = 0x00000020,
    Persistent = 0x00000400,
    Ignored = 0x00001000,
    Blocked = 0x00002000,
}

pub type StringSizeType = u32;

#[derive(Debug, Clone, Copy, Hash)]
pub struct FixedString<const N: usize> {
    pub data: [u8; N],
}

impl<const N: usize> Default for FixedString<N> {
    fn default() -> Self {
        Self { data: [0; N] }
    }
}

impl<const N: usize> FixedString<N> {
    pub const S_CAPACITY: usize = N;

    pub fn new(s: &str) -> Self {
        let mut data = [0u8; N];
        let bytes = s.as_bytes();
        let len = std::cmp::min(bytes.len(), N);
        data[..len].copy_from_slice(&bytes[..len]);
        if N != 4 && len < N {
            data[len] = 0;
        } else if N != 4 && N > 0 && len == N {
            data[N - 1] = 0;
        }
        Self { data }
    }
    pub fn as_bytes(&self) -> &[u8; N] {
        &self.data
    }
    pub fn from_le_bytes(bytes: [u8; N]) -> Self {
        Self { data: bytes }
    }

    pub fn to_string_view(&self) -> &str {
        let len = self.data.iter().position(|&b| b == 0).unwrap_or(N);
        str::from_utf8(&self.data[..len]).unwrap_or_default()
    }

    pub fn to_string_lossy(&self) -> String {
        let len = self.data.iter().position(|&b| b == 0).unwrap_or(N);
        String::from_utf8_lossy(&self.data[..len]).to_string() // Use .to_string() or .into_owned() on Cow
    }

    pub fn clear(&mut self) {
        self.data = [0; N];
    }

    pub fn assign(&mut self, value: &str) {
        let bytes = value.as_bytes();
        let len_to_copy = std::cmp::min(bytes.len(), N);

        self.data.fill(0);
        self.data[..len_to_copy].copy_from_slice(&bytes[..len_to_copy]);

        if N != 4 {
            if len_to_copy < N {
            } else if N > 0 {
                self.data[N - 1] = 0;
            }
        }
    }
}

impl FixedString<4> {
    pub fn from_u32(value: u32) -> Self {
        Self {
            data: value.to_le_bytes(),
        }
    }

    pub fn to_int(&self) -> u32 {
        u32::from_le_bytes(self.data)
    }
}

impl<const N: usize> PartialEq for FixedString<N> {
    fn eq(&self, other: &Self) -> bool {
        self.data[..] == other.data[..]
    }
}
impl<const N: usize> Eq for FixedString<N> {}

impl<const N: usize> PartialEq<&str> for FixedString<N> {
    fn eq(&self, other: &&str) -> bool {
        let self_len = self.data.iter().position(|&b| b == 0).unwrap_or(N);
        let other_bytes = other.as_bytes();

        if self_len != other_bytes.len() {
            return false;
        }
        self.data[..self_len] == other_bytes[..self_len]
    }
}

impl PartialEq<u32> for FixedString<4> {
    fn eq(&self, other: &u32) -> bool {
        self.to_int() == *other
    }
}

pub type Name = FixedString<4>;
pub type Name32 = FixedString<32>;
pub type Name64 = FixedString<64>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EsmContext {
    pub filename: PathBuf,
    pub remaining_rec_data: i64,
    pub remaining_subrec_data: u32,
    pub remaining_file_data: u64,
    pub rec_name: Name,
    pub sub_name: Name,
    pub index: usize,
    pub parent_file_indices: Vec<usize>,
    pub sub_cached: bool,
    pub file_pos: u64,
}

impl EsmContext {
    pub fn clear(&mut self) {
        self.filename.clear();
        self.remaining_rec_data = 0;
        self.remaining_subrec_data = 0;
        self.remaining_file_data = 0;
        self.rec_name.clear();
        self.sub_name.clear();
        self.parent_file_indices.clear();
        self.sub_cached = false;
        self.file_pos = 0;
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EsmVersion {
    pub ui: u32,
    pub f: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FormatVersion(pub u32);

pub const MAX_STRING_REF_ID_FORMAT_VERSION_CONST: FormatVersion = FormatVersion(1);
pub const MAX_LIMITED_SIZE_STRINGS_FORMAT_VERSION_CONST: FormatVersion = FormatVersion(1);
pub const MAX_USE_ESM_CELL_ID_FORMAT_VERSION_CONST: FormatVersion = FormatVersion(1);

#[macro_export]
macro_rules! static_assert {
    ($condition:expr) => {
        const _: () = assert!($condition);
    };
    ($condition:expr, $message:expr) => {
        const _: () = assert!($condition, $message);
    };
}

fn _size_checks() {
    assert_eq!(std::mem::size_of::<Name>(), 4);
    assert_eq!(std::mem::size_of::<Name32>(), 32);
    assert_eq!(std::mem::size_of::<Name64>(), 64);
}
