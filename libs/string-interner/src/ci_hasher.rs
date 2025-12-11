use core::hash::{BuildHasher, Hasher};

/// A hasher that converts characters to ASCII lowercase before hashing.
#[derive(Default)]
pub struct CaseInsensitiveHasher {
    state: u64,
}

impl Hasher for CaseInsensitiveHasher {
    fn write_u8(&mut self, i: u8) {
        // Convert to lowercase before hashing the byte
        self.state = self.state.wrapping_add(i.to_ascii_lowercase() as u64);
    }

    fn write(&mut self, bytes: &[u8]) {
        self.state = self.state.wrapping_add(
            bytes
                .iter()
                .map(|b| b.to_ascii_lowercase())
                .fold(0u64, |acc, b| acc.wrapping_add(b as u64)),
        );
    }

    fn finish(&self) -> u64 {
        self.state
    }
}

/// A BuildHasher for creating CaseInsensitiveHasher instances.
#[derive(Default)]
pub struct CaseInsensitiveBuildHasher;

impl BuildHasher for CaseInsensitiveBuildHasher {
    type Hasher = CaseInsensitiveHasher;

    fn build_hasher(&self) -> Self::Hasher {
        CaseInsensitiveHasher::default()
    }
}
