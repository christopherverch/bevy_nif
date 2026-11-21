//! Morrowind `.bsa` archive reader (original, uncompressed format).
//!
//! The stock Morrowind/Tribunal/Bloodmoon data ships almost everything
//! (textures, meshes, sounds, ...) packed inside `Morrowind.bsa`,
//! `Tribunal.bsa` and `Bloodmoon.bsa`. Loose files on disk always win;
//! this module provides a case-insensitive index over all `.bsa` files
//! found under `<asset root>/Data Files` so both path resolution
//! (`check_exists` / `resolve_bevy_asset_path`) and the client's BSA-aware
//! asset reader can find archive-packed entries.
//!
//! Format (the *original* Morrowind format, `BsaVersion::Uncompressed`,
//! as parsed by OpenMW in `components/bsa/bsafile.cpp`; NOT the
//! later `BSA\0` format used by TES4 and up):
//!
//! ```text
//! u32 version    (= 0x100)
//! u32 dirsize    (bytes in the directory block, starting at byte 12)
//! u32 numfiles
//! directory block:
//!   u32 fileSize, u32 offset        (numfiles pairs, little-endian;
//!                                     offset is relative to the data buffer)
//!   u32 name offset                 (numfiles values, relative to the
//!                                     start of the name buffer)
//!   name buffer                     (dirsize - 12*numfiles bytes;
//!                                     NUL-terminated, backslash paths)
//! hash block (8*numfiles bytes, ignored by OpenMW and here)
//! data buffer (to end of file): the actual file payloads; an entry's
//!   absolute file offset is 12 + dirsize + 8*numfiles + offset.
//!
//! Payloads are stored verbatim (no compression).
#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashMap;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Version field of the original (uncompressed) Morrowind BSA format.
const BSA_VERSION_UNCOMPRESSED: u32 = 0x100;

/// One entry in a BSA index.
#[derive(Debug, Clone)]
pub struct BsaEntry {
    /// Normalized entry name (lowercase, `/` separators).
    pub name: String,
    /// Original name as stored in the archive.
    pub original_name: String,
    /// Absolute byte offset of the payload from the start of the archive file.
    pub offset: u64,
    /// Payload size in bytes.
    pub size: u64,
}

/// Errors while parsing or reading BSA archives.
#[derive(Debug)]
pub enum BsaError {
    Io(std::io::Error),
    /// File is not an uncompressed Morrowind-format BSA (wrong version field).
    BadMagic(PathBuf),
    /// Archive header is inconsistent with the on-disk file size.
    BadHeader(PathBuf, String),
    TruncatedIndex(PathBuf, String),
    EntryNotFound(String),
}

impl std::fmt::Display for BsaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BsaError::Io(e) => write!(f, "I/O error: {e}"),
            BsaError::BadMagic(p) => {
                write!(f, "not an uncompressed Morrowind BSA file: {}", p.display())
            }
            BsaError::BadHeader(p, msg) => write!(f, "{}: {msg}", p.display()),
            BsaError::TruncatedIndex(p, msg) => write!(f, "{}: {msg}", p.display()),
            BsaError::EntryNotFound(name) => write!(f, "entry not found in any BSA: {name}"),
        }
    }
}

impl std::error::Error for BsaError {}

impl From<std::io::Error> for BsaError {
    fn from(e: std::io::Error) -> Self {
        BsaError::Io(e)
    }
}

/// A single parsed archive.
pub struct BsaArchive {
    path: PathBuf,
    /// Load-priority rank (higher wins). Morrowind=0, Tribunal=1,
    /// Bloodmoon=2, anything else=3.
    priority: u8,
    entries: HashMap<String, BsaEntry>,
    /// Lazily opened, shared file handle.
    file: Mutex<Option<BufReader<std::fs::File>>>,
}

impl BsaArchive {
    fn load(path: &Path) -> Result<Self, BsaError> {
        let mut f = std::fs::File::open(path)?;
        let file_len = f.metadata()?.len();
        let mut header = [0u8; 12];
        f.read_exact(&mut header)
            .map_err(|e| BsaError::TruncatedIndex(path.to_path_buf(), e.to_string()))?;
        let version = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        if version != BSA_VERSION_UNCOMPRESSED {
            return Err(BsaError::BadMagic(path.to_path_buf()));
        }
        let dirsize = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as u64;
        let numfiles = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as u64;

        let n = numfiles as usize;
        let pairs_size = 8 * n as u64;
        let nameoffs_size = 4 * n as u64;
        let dir_end = 12u64 + dirsize;
        let data_start = dir_end + 8 * n as u64;
        if dirsize < pairs_size + nameoffs_size {
            return Err(BsaError::BadHeader(
                path.to_path_buf(),
                format!(
                    "dirsize {dirsize} smaller than minimum index size {}",
                    pairs_size + nameoffs_size
                ),
            ));
        }
        if file_len < data_start {
            return Err(BsaError::BadHeader(
                path.to_path_buf(),
                format!(
                    "file is {file_len} bytes but header requires at least {data_start} bytes (data buffer start)"
                ),
            ));
        }

        // Directory block: (size, offset) pairs, name offsets, then names.
        // `dir` holds file bytes [12, 12+dirsize), so slice indices are
        // relative to byte 12 (NOT the absolute offsets above).
        let mut dir = vec![0u8; dirsize as usize];
        f.seek(SeekFrom::Start(12))?;
        f.read_exact(&mut dir)
            .map_err(|e| BsaError::TruncatedIndex(path.to_path_buf(), e.to_string()))?;
        drop(f);

        let nameoffs_idx = pairs_size as usize;
        let namebuf_idx = (pairs_size + nameoffs_size) as usize;
        let pairs = &dir[..nameoffs_idx];
        let nameoffs = &dir[nameoffs_idx..namebuf_idx];
        let namebuf = &dir[namebuf_idx..];

        let mut entries: HashMap<String, BsaEntry> = HashMap::with_capacity(n);
        for i in 0..n {
            let file_size = u32::from_le_bytes([
                pairs[8 * i],
                pairs[8 * i + 1],
                pairs[8 * i + 2],
                pairs[8 * i + 3],
            ]) as u64;
            let rel_offset = u32::from_le_bytes([
                pairs[8 * i + 4],
                pairs[8 * i + 5],
                pairs[8 * i + 6],
                pairs[8 * i + 7],
            ]) as u64;
            let nameoff = u32::from_le_bytes([
                nameoffs[4 * i],
                nameoffs[4 * i + 1],
                nameoffs[4 * i + 2],
                nameoffs[4 * i + 3],
            ]) as usize;
            if nameoff >= namebuf.len() {
                log::warn!(
                    "bevy_nif::bsa: {}: entry {i} name offset {nameoff} out of range; skipping",
                    path.display()
                );
                continue;
            }
            let rest = &namebuf[nameoff..];
            let nul = rest.iter().position(|&b| b == 0).ok_or_else(|| {
                BsaError::TruncatedIndex(
                    path.to_path_buf(),
                    format!("entry {i}: unterminated name at name offset {nameoff}"),
                )
            })?;
            let original_name = String::from_utf8_lossy(&rest[..nul]).into_owned();
            let entry = BsaEntry {
                name: normalize_bsa_name(&original_name),
                original_name,
                offset: data_start + rel_offset,
                size: file_size,
            };
            if entry.offset + entry.size > file_len {
                log::warn!(
                    "bevy_nif::bsa: {}: entry {} ({} bytes @ 0x{:x}) extends past EOF; skipping",
                    path.display(),
                    entry.name,
                    entry.size,
                    entry.offset
                );
                continue;
            }
            // Later entries for the same name replace earlier ones.
            entries.insert(entry.name.clone(), entry);
        }

        Ok(Self {
            path: path.to_path_buf(),
            priority: archive_priority(path),
            entries,
            file: Mutex::new(None),
        })
    }

    /// Case-insensitive exact-name lookup.
    pub fn find(&self, name: &str) -> Option<&BsaEntry> {
        self.entries.get(&normalize_bsa_name(name))
    }

    /// Reads the payload of an entry by exact name.
    pub fn read_entry(&self, name: &str) -> Result<Vec<u8>, BsaError> {
        let entry = self
            .find(name)
            .ok_or_else(|| BsaError::EntryNotFound(name.to_string()))?;
        let mut guard = self.open()?;
        let f = guard.as_mut().expect("open() guarantees Some");
        f.seek(SeekFrom::Start(entry.offset))?;
        let mut buf = vec![0u8; entry.size as usize];
        f.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Lazily opens the archive file (once) and returns the guard.
    fn open(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<BufReader<std::fs::File>>>, BsaError> {
        let mut guard = self.file.lock().map_err(|e| {
            BsaError::Io(std::io::Error::other(format!("BSA file lock poisoned: {e}")))
        })?;
        if guard.is_none() {
            let f = std::fs::File::open(&self.path)?;
            *guard = Some(BufReader::with_capacity(1024 * 1024, f));
        }
        Ok(guard)
    }
}

/// All scanned archives, kept in load-priority order.
pub struct BsaArchiveSet {
    /// Lowest priority first (so reverse iteration finds the winner).
    archives: Vec<BsaArchive>,
}

impl BsaArchiveSet {
    /// Scans `dir` for `*.bsa` (case-insensitive) and indexes them all.
    pub fn scan_dir(dir: &Path) -> Self {
        let mut found = Vec::new();
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("BSA"))
                {
                    match BsaArchive::load(&path) {
                        Ok(archive) => {
                            log::info!(
                                "bevy_nif::bsa: indexed {} ({} entries)",
                                path.display(),
                                archive.entries.len()
                            );
                            found.push(archive);
                        }
                        Err(e) => {
                            log::warn!("bevy_nif::bsa: failed to load {}: {e}", path.display());
                        }
                    }
                }
            }
        } else {
            log::warn!(
                "bevy_nif::bsa: no BSA directory at {} — archive-packed assets unavailable",
                dir.display()
            );
        }
        found.sort_by_key(|a| a.priority);
        Self { archives: found }
    }

    /// Case-insensitive lookup. When the same name exists in several
    /// archives, the highest-priority archive (base game < Tribunal <
    /// Bloodmoon) wins, mirroring the game's load order.
    pub fn find(&self, name: &str) -> Option<&BsaEntry> {
        self.archives.iter().rev().find_map(|a| a.find(name))
    }

    /// Like [`BsaArchiveSet::find`] but also tries progressively shorter
    /// path suffixes, so both `"textures/landscape/dune.dds"` and
    /// `"Data Files/Textures/Landscape/dune.dds"` resolve.
    pub fn find_any(&self, path: &str) -> Option<&BsaEntry> {
        let normalized = normalize_bsa_name(path);
        let mut rest = normalized.as_str();
        loop {
            if let Some(e) = self.find(rest) {
                return Some(e);
            }
            let i = rest.find('/')?;
            rest = &rest[i + 1..];
        }
    }

    /// Reads the payload for a query, with the same suffix-tolerant
    /// matching as [`BsaArchiveSet::find_any`].
    pub fn read_entry_any(&self, path: &str) -> Result<Vec<u8>, BsaError> {
        let normalized = normalize_bsa_name(path);
        let mut rest = normalized.as_str();
        loop {
            for archive in self.archives.iter().rev() {
                if archive.find(rest).is_some() {
                    return archive.read_entry(rest);
                }
            }
            match rest.find('/') {
                Some(i) => rest = &rest[i + 1..],
                None => return Err(BsaError::EntryNotFound(normalized)),
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.archives.is_empty()
    }

    pub fn archive_count(&self) -> usize {
        self.archives.len()
    }

    /// Case-insensitive file-name (basename) lookup across every entry in
    /// every archive, highest-priority archive first.
    ///
    /// The original Morrowind engine indexes BSA files by a hash of the file
    /// *name* (the 8-byte hash block after the directory table), so a lookup
    /// for `B_N_Orc_M_Skins.nif` succeeds no matter which subdirectory the
    /// BSA builder stored it under — this data's Morrowind.bsa keeps meshes in
    /// arbitrary subfolders such as `meshes/b/`, which
    /// [`BsaArchiveSet::find_any`]'s suffix-stripping cannot recover.
    pub fn find_by_file_name(&self, name: &str) -> Option<&BsaEntry> {
        let name = normalize_bsa_name(name);
        let basename = name.rsplit('/').next().unwrap_or(&name);
        self.archives.iter().rev().find_map(|archive| {
            archive.entries.values().find(|entry| {
                entry
                    .name
                    .rsplit('/')
                    .next()
                    .is_some_and(|stored| stored.eq_ignore_ascii_case(basename))
            })
        })
    }

    /// Reads the payload for a basename query, with the same priority
    /// semantics as [`BsaArchiveSet::find_by_file_name`].
    pub fn read_entry_by_file_name(&self, name: &str) -> Result<Vec<u8>, BsaError> {
        let name = normalize_bsa_name(name);
        let basename = name.rsplit('/').next().unwrap_or(&name);
        for archive in self.archives.iter().rev() {
            if let Some(entry) = archive.entries.values().find(|entry| {
                entry
                    .name
                    .rsplit('/')
                    .next()
                    .is_some_and(|stored| stored.eq_ignore_ascii_case(basename))
            }) {
                return archive.read_entry(&entry.name);
            }
        }
        Err(BsaError::EntryNotFound(name))
    }
}

impl std::fmt::Debug for BsaArchive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BsaArchive")
            .field("path", &self.path)
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl std::fmt::Debug for BsaArchiveSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BsaArchiveSet")
            .field("archives", &self.archives.len())
            .finish()
    }
}

/// Load-priority rank: base game first, then DLC in release order.
fn archive_priority(path: &Path) -> u8 {
    match path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_ascii_lowercase())
        .as_deref()
    {
        Some("morrowind.bsa") => 0,
        Some("tribunal.bsa") => 1,
        Some("bloodmoon.bsa") => 2,
        _ => 3,
    }
}

/// Lowercase + `/` separators — Morrowind asset names are case-insensitive.
pub fn normalize_bsa_name(name: &str) -> String {
    name.replace('\\', "/").to_ascii_lowercase()
}

static SET: OnceLock<BsaArchiveSet> = OnceLock::new();

/// Process-wide archive set, lazily built from `<asset root>/Data Files`.
/// The client pre-initializes this before the first asset load; the lazy
/// fallback uses bevy_nif's own asset-root resolution so the helper
/// functions work standalone.
pub fn bsa_set() -> &'static BsaArchiveSet {
    SET.get_or_init(|| {
        let dir = crate::helper_funcs::asset_root().join("Data Files");
        BsaArchiveSet::scan_dir(&dir)
    })
}

/// Eagerly initializes the archive set from an explicit directory.
/// Only the first call wins (OnceLock semantics).
pub fn init_bsa_set(dir: &Path) -> &'static BsaArchiveSet {
    let _ = SET.get_or_init(|| BsaArchiveSet::scan_dir(dir));
    SET.get().expect("SET initialized above")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Small temp-dir helper for tests.
#[cfg(test)]
fn test_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bsa_test_{label}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Builds a minimal uncompressed (Morrowind-format) BSA file.
#[cfg(test)]
fn write_bsa(path: &Path, entries: &[(&str, &[u8])]) {
    let n = entries.len() as u32;
    let mut namebuf = Vec::new();
    let mut nameoffs: Vec<u32> = Vec::with_capacity(entries.len());
    for (name, _) in entries {
        nameoffs.push(namebuf.len() as u32);
        namebuf.extend_from_slice(name.as_bytes());
        namebuf.push(0);
    }
    let dirsize = 8 * n as u64 + 4 * n as u64 + namebuf.len() as u64;

    let mut pairs = Vec::with_capacity(8 * entries.len());
    let mut payload = Vec::new();
    for (_, data) in entries {
        pairs.extend_from_slice(&(data.len() as u32).to_le_bytes()); // fileSize first
        pairs.extend_from_slice(&(payload.len() as u32).to_le_bytes()); // offset relative to data buffer
        payload.extend_from_slice(data);
    }

    let mut out = Vec::new();
    out.extend_from_slice(&BSA_VERSION_UNCOMPRESSED.to_le_bytes());
    out.extend_from_slice(&(dirsize as u32).to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&pairs);
    for off in &nameoffs {
        out.extend_from_slice(&off.to_le_bytes());
    }
    out.extend_from_slice(&namebuf);
    let hash_len = 8 * entries.len();
    out.extend(std::iter::repeat_n(0u8, hash_len)); // hash block (ignored)
    out.extend_from_slice(&payload);
    std::fs::write(path, &out).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_is_case_and_separator_insensitive() {
        assert_eq!(
            normalize_bsa_name("Data\\Files\\Textures/Landscape/DUNE.DDS"),
            "data/files/textures/landscape/dune.dds"
        );
    }

    #[test]
    fn bad_magic_rejected() {
        let dir = test_dir("magic");
        // "BSA\0"-style magic (TES4-era) must be rejected by this reader.
        let p = dir.join("not.bsa");
        let mut bytes = b"BSA\0".to_vec();
        bytes.extend_from_slice(&[0u8; 8]);
        std::fs::write(&p, bytes).unwrap();
        let err = BsaArchive::load(&p).unwrap_err();
        assert!(matches!(err, BsaError::BadMagic(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn truncated_header_rejected() {
        let dir = test_dir("trunc");
        let p = dir.join("trunc.bsa");
        // Valid version field, but the file ends before the 12-byte header.
        std::fs::write(&p, [0x00u8, 0x01, 0x00, 0x00]).unwrap();
        let err = BsaArchive::load(&p).unwrap_err();
        assert!(
            matches!(err, BsaError::TruncatedIndex(_, _)),
            "expected TruncatedIndex, got {err:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn roundtrip_reads_back_entries() {
        let dir = test_dir("rt");
        let p = dir.join("test.bsa");
        let a = b"hello stored payload";
        let b: Vec<u8> = vec![b'z'; 4096];
        write_bsa(
            &p,
            &[
                ("textures\\stored.dds", a),
                ("Textures\\Compressed.dds", b.as_slice()),
            ],
        );
        let set = BsaArchiveSet::scan_dir(&dir);
        assert_eq!(set.archive_count(), 1);
        // case-insensitive + separator-insensitive lookup
        assert!(set.find("TEXTURES/STORED.dds").is_some());
        let data = set.read_entry_any("textures/stored.dds").unwrap();
        assert_eq!(data, a);
        let data = set
            .read_entry_any("Data Files/Textures\\COMPRESSED.dDs")
            .unwrap();
        assert_eq!(data, b);
        // suffix stripping: "data files/" prefix is not part of BSA names
        assert!(set.find_any("Data Files/Textures/stored.dds").is_some());
        assert!(set.find("nope/nothing.bsa").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn entry_with_space_in_name() {
        // Real Morrowind BSAs contain names with spaces
        // (e.g. "meshes\\a\\a_ice minions_shield2.nif").
        let dir = test_dir("space");
        let p = dir.join("test.bsa");
        write_bsa(&p, &[("meshes\\a\\a ice minion.nif", b"payload!")]);
        let set = BsaArchiveSet::scan_dir(&dir);
        let data = set.read_entry_any("meshes/a/a ice minion.nif").unwrap();
        assert_eq!(data, b"payload!");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn basename_lookup_recovers_nested_entries() {
        // This data's BSA stores meshes in arbitrary subfolders
        // (e.g. `meshes\m\probe_journeyman_01.nif`), which find_any's
        // suffix-stripping cannot recover — only a basename search can.
        let dir = test_dir("basename");
        let p = dir.join("test.bsa");
        write_bsa(
            &p,
            &[
                ("meshes\\m\\probe_journeyman_01.nif", b"nif-payload"),
                ("meshes\\base_anim.nif", b"anim-payload"),
            ],
        );
        let set = BsaArchiveSet::scan_dir(&dir);
        // find_any must NOT find the nested entry from a flat query...
        assert!(set.find_any("meshes/probe_journeyman_01.nif").is_none());
        // ...but the basename lookup does.
        let entry = set
            .find_by_file_name("Probe_Journeyman_01.NIF")
            .expect("basename lookup must find nested entry");
        assert_eq!(entry.name, "meshes/m/probe_journeyman_01.nif");
        let data = set.read_entry_by_file_name("probe_journeyman_01.nif").unwrap();
        assert_eq!(data, b"nif-payload");
        // Plain entries resolve by basename too.
        assert_eq!(
            set.read_entry_by_file_name("base_anim.nif").unwrap(),
            b"anim-payload"
        );
        // Misses stay misses.
        assert!(set.find_by_file_name("not_there.nif").is_none());
        assert!(set.read_entry_by_file_name("not_there.nif").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn later_archives_win_by_priority() {
        let dir = test_dir("prio");
        let payload_a = b"from morrowind";
        let payload_b = b"from bloodmoon";
        write_bsa(&dir.join("Morrowind.bsa"), &[("x\\t.dds", payload_a)]);
        write_bsa(&dir.join("Bloodmoon.bsa"), &[("x\\t.dds", payload_b)]);
        let set = BsaArchiveSet::scan_dir(&dir);
        assert_eq!(set.archive_count(), 2);
        let data = set.read_entry_any("x/t.dds").unwrap();
        assert_eq!(data, payload_b, "Bloodmoon (later) must win");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Integration: parse the real `Morrowind.bsa` when the game data is
    /// present (gitignored, so skip otherwise).
    #[test]
    fn real_morrowind_bsa_landscape_texture() {
        let candidates = [
            PathBuf::from("assets/Data Files/Morrowind.bsa"),
            PathBuf::from("../../apps/client/assets/Data Files/Morrowind.bsa"),
            PathBuf::from("../../tes3mp_rust/apps/client/assets/Data Files/Morrowind.bsa"),
        ];
        let Some(path) = candidates.into_iter().find(|p| p.exists()) else {
            eprintln!("Morrowind.bsa not found; skipping integration test");
            return;
        };
        let archive = BsaArchive::load(&path).expect("Morrowind.bsa must parse");
        assert!(
            archive.entries.len() > 10_000,
            "expected the base game's ~11k entries, got {}",
            archive.entries.len()
        );
        // Landscape textures ship as `textures\<stem>.dds` (the ESM's LTEX
        // records name them as bare `.tga` — the client applies the mapping).
        let entry = archive
            .find("textures/tx_sand_01.dds")
            .expect("textures/tx_sand_01.dds must exist");
        let data = archive.read_entry("textures/tx_sand_01.dds").unwrap();
        assert_eq!(data.len(), entry.size as usize);
        assert_eq!(&data[..4], b"DDS ", "payload must be a verbatim DDS file");
        // DDS header: dwHeight @12, dwWidth @16 (256x256 verified by probe).
        let (h, w) = (
            u32::from_le_bytes([data[12], data[13], data[14], data[15]]),
            u32::from_le_bytes([data[16], data[17], data[18], data[19]]),
        );
        assert_eq!((h, w), (256, 256));
        // Set-level lookup with suffix tolerance must also work.
        let set = BsaArchiveSet::scan_dir(path.parent().unwrap());
        assert!(set.find_any("Textures\\TX_sand_01.DDS").is_some());
        // Character meshes sit in arbitrary subfolders (meshes\b\...), so
        // suffix-tolerant lookup misses them; the basename fallback
        // (mirroring the engine's name-hash file search) must find them.
        assert!(set.find_any("meshes/B_N_Orc_M_Skins.nif").is_none());
        let entry = set
            .find_by_file_name("B_N_Orc_M_Skins.nif")
            .expect("B_N_Orc_M_Skins.nif must resolve by basename");
        assert_eq!(entry.name, "meshes/b/b_n_orc_m_skins.nif");
        let data = set
            .read_entry_by_file_name("B_N_Orc_M_Skins.nif")
            .unwrap();
        assert_eq!(data.len(), entry.size as usize);
        // This data's NIFs use the 40-byte NetImmerse ASCII header
        // (see libs/nif HEADER), not the classic `\0NIF` magic.
        assert!(data.starts_with(b"NetImmerse File Format, Version 4.0.0.2\n"));
        eprintln!(
            "integration: indexed {} entries from {} ({} archives)",
            archive.entries.len(),
            path.display(),
            set.archive_count()
        );
    }
}
