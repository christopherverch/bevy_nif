// src/lib.rs

// --- Core modules ---
extern crate bitflags;
pub mod common_types;
pub mod esm_reader;
pub mod format_version_defs;
pub mod header_types;
pub mod name_defs;
pub mod readers_cache;
pub mod refid_types;
mod string_interning;
pub mod traits;
pub mod types;
pub use traits::*;

// Core types
pub use common_types::{EsmContext, EsmVersion, FixedString, Name4, Name8, StringSizeType};
pub use esm_reader::{EsmError, EsmReader};
pub use header_types::{Data as EsmFileHeaderData, Gmdt, Header, MasterData};
pub use refid_types::{
    ESM3ExteriorCellRefId, EmptyRefId, FormId, GeneratedRefId, IndexRefId, RefId, RefIdType,
    StringRefId,
};
pub use types::cell_id::*;
pub use types::landscape::*;
pub use types::landscape_textures::*;
pub use types::weapon::*;
