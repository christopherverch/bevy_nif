use bytemuck::{Pod, Zeroable};
use core::fmt;
use glam::Vec3;
use std::path::PathBuf;
use std::str;

use esm_parser_macros::LoadSave;
#[derive(LoadSave, Clone, Debug, Default, PartialEq)]
pub struct Position {
    pub pos: Vec3,
    pub rot: Vec3, // Euler angles in radians (X, Y, Z)
}
pub type StringSizeType = u32;

#[derive(Debug, Clone, Copy, Hash)]
pub struct FixedString<const N: usize> {
    pub data: [u8; N],
}
unsafe impl<const N: usize> Zeroable for FixedString<N> {}
unsafe impl<const N: usize> Pod for FixedString<N> {}

impl<const N: usize> Default for FixedString<N> {
    fn default() -> Self {
        Self { data: [0; N] }
    }
}
/// Creates a const Name4 from a string, which is compiled to the byte equivalent
#[macro_export]
macro_rules! name4 {
    ($s:literal) => {{
        // Use the const constructor to create the FixedString<4> at compile time.
        $crate::Name4::new_const($s)
    }};
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
    pub const fn new_const(s: &'static str) -> Self {
        let bytes = s.as_bytes();
        let s_len = bytes.len();

        let mut data = [0u8; N];

        // Determine the number of bytes to copy (up to N)
        let len_to_copy = if s_len < N { s_len } else { N };

        // Manually copy bytes
        let mut i = 0;
        while i < len_to_copy {
            data[i] = bytes[i];
            i += 1;
        }

        // null-termination logic for strings that fit:
        // C++ FixedString<4> is a special case (no null terminator).
        if N != 4 {
            if len_to_copy < N {
                // Case 1: string fits and is shorter than capacity. Null-terminate
                data[len_to_copy] = 0;
            } else if N > 0 {
                // Case 2: string is exactly N. Null-terminate by overwriting the last byte.
                data[N - 1] = 0;
            }
        }

        Self { data }
    }
    pub fn as_bytes(&self) -> &[u8; N] {
        &self.data
    }
    pub fn from_le_bytes(bytes: [u8; N]) -> Self {
        Self { data: bytes }
    }

    pub fn to_str(&self) -> &str {
        let len = self.data.iter().position(|&b| b == 0).unwrap_or(N);
        str::from_utf8(&self.data[..len]).unwrap_or_default()
    }

    pub fn to_string_lossy(&self) -> String {
        let len = self.data.iter().position(|&b| b == 0).unwrap_or(N);
        String::from_utf8_lossy(&self.data[..len]).to_string()
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

    pub const fn to_u32(&self) -> u32 {
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
        self.to_u32() == *other
    }
}

pub type Name4 = FixedString<4>;
pub type Name8 = FixedString<8>;
pub type Name32 = FixedString<32>;
pub type Name64 = FixedString<64>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EsmContext {
    pub filename: PathBuf,
    // Maybe remaining rec data should be i64 so we can go past a record and then backtrack?
    pub remaining_rec_data: u32,
    pub remaining_subrec_data: u32,
    pub rec_name: Name4,
    pub sub_name: Name4,
    // When working with multiple esX files, we will generate lists of all files that
    // actually contribute to a specific cell. Therefore, we need to store the index
    // of the file belonging to this contest. Further details will be in esm_store crate in
    // Store<Cell>, but not currently implemented
    pub index: i16,
    pub parent_file_indices: Vec<usize>,
    /// True if subName has been read but not used.
    pub sub_cached: bool,
    /// File position. Only used for stored contexts, not regularly
    /// updated within the reader itself.
    pub file_pos: u64,
}

impl EsmContext {
    pub fn clear(&mut self) {
        self.filename.clear();
        self.remaining_rec_data = 0;
        self.remaining_subrec_data = 0;
        self.sub_cached = false;
        self.rec_name.clear();
        self.sub_name.clear();
    }
}

#[derive(Clone, Copy)]
pub union EsmVersion {
    pub f: f32,
    pub ui: u32,
}
impl Default for EsmVersion {
    fn default() -> Self {
        EsmVersion { ui: 0 }
    }
}
impl fmt::Debug for EsmVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe { write!(f, "EsmVersion {{ f: {}, ui: {:?} }}", self.f, self.ui) }
    }
}
/// Utility macro, we can do things like static_assert(1+1=2) and have it evaluate at compile time
#[macro_export]
macro_rules! static_assert {
    ($condition:expr) => {
        const _: () = assert!($condition);
    };
    ($condition:expr, $message:expr) => {
        const _: () = assert!($condition, $message);
    };
}
