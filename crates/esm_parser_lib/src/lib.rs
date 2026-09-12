// Core modules
extern crate bitflags;
mod common_consts;
pub mod common_inventory_types;
pub mod common_types;
pub mod esm_reader;
pub mod esm_record;
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
pub use string_interning::debug_interns;
pub use string_interning::find_interned;
pub use types::bodypart::*;
pub use types::cell_id::*;
pub use types::landscape::*;
pub use types::landscape_textures::*;
pub use types::magic_effect::MAGIC_EFFECT_ID_TO_INDEX;
pub use types::weapon::*;
