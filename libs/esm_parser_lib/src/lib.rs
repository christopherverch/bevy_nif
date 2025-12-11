// src/lib.rs

// --- Core modules ---
extern crate bitflags;
pub mod common_types;
pub mod esm_reader;
pub mod format_version_defs;
pub mod header_types;
mod macros;
pub mod name_defs;
pub mod readers_cache;
pub mod refid_types;
pub mod traits;
pub mod types;
pub use traits::*;
// --- Helper/shared data structure modules ---

// Core types
pub use common_types::{EsmContext, EsmVersion, FixedString, Name32, Name64, StringSizeType};
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

// Helper/shared data structure types
