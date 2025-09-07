// src/lib.rs

// --- Core modules ---
extern crate bitflags;
pub mod common_types;
pub mod esm_reader;
pub mod esm_store;
pub mod header_types;
mod macros;
pub mod readers_cache;
pub mod refid_types;
pub mod traits;
pub mod types;
pub use esm_store::*;
pub use traits::*;
// --- Helper/shared data structure modules ---

// Core types
pub use common_types::{
    EsmContext, EsmVersion, FixedString, FormatVersion,
    MAX_LIMITED_SIZE_STRINGS_FORMAT_VERSION_CONST, MAX_STRING_REF_ID_FORMAT_VERSION_CONST,
    MAX_USE_ESM_CELL_ID_FORMAT_VERSION_CONST, Name32, Name64, StringSizeType,
};
pub use esm_reader::{EsmError, EsmReader, TextDecoder};
pub use header_types::{Data as EsmFileHeaderData, Gmdt, Header, MasterData};
pub use refid_types::{
    ESM3ExteriorCellRefId, EmptyRefId, FormId, GeneratedRefId, IndexRefId, RefId, RefIdType,
    RefIdValue, StringRefId,
};
pub use types::cell::*;
pub use types::landscape::*;
pub use types::landscape_textures::*;
pub use types::weapon::*;

// Helper/shared data structure types
