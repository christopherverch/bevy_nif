// src/lib.rs

// --- Core modules ---
pub mod common_types;
pub mod esm_reader;
pub mod header_types;
pub mod loadland;
pub mod loadltex;
pub mod loadweap;
pub mod refid_types;
// --- Helper/shared data structure modules ---

// Core types
pub use common_types::{
    EsmContext, EsmVersion, FixedString, FormatVersion,
    MAX_LIMITED_SIZE_STRINGS_FORMAT_VERSION_CONST, MAX_STRING_REF_ID_FORMAT_VERSION_CONST,
    MAX_USE_ESM_CELL_ID_FORMAT_VERSION_CONST, Name, Name32, Name64, RecordFlag, StringSizeType,
};
pub use esm_reader::{EsmError, EsmReader, SimpleTextDecoder, TextDecoder};
pub use header_types::{Data as EsmFileHeaderData, Gmdt, Header, MasterData};
pub use loadland::*;
pub use loadltex::*;
pub use refid_types::{
    ESM3ExteriorCellRefId, EmptyRefId, FormId, GeneratedRefId, IndexRefId, RefId, RefIdType,
    RefIdValue, StringRefId,
};

// Helper/shared data structure types
