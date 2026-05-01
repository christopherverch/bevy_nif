pub type FormatVersion = u32;

// Current content format version
pub const DEFAULT_FORMAT_VERSION: FormatVersion = 0;
pub const CURRENT_CONTENT_FORMAT_VERSION: FormatVersion = 1;
pub const MAX_OLD_GOLD_VALUE_FORMAT_VERSION: FormatVersion = 5;
pub const MAX_OLD_FOG_OF_WAR_FORMAT_VERSION: FormatVersion = 6;
pub const MAX_UNOPTIMIZED_CHARACTER_DATA_FORMAT_VERSION: FormatVersion = 7;
pub const MAX_OLD_TIME_LEFT_FORMAT_VERSION: FormatVersion = 8;
pub const MAX_INT_FALLBACK_FORMAT_VERSION: FormatVersion = 10;
pub const MAX_OLD_RESTOCKING_FORMAT_VERSION: FormatVersion = 14;
pub const MAX_CLEAR_MODIFIERS_FORMAT_VERSION: FormatVersion = 16;
pub const MAX_OLD_AI_PACKAGE_FORMAT_VERSION: FormatVersion = 17;
pub const MAX_OLD_SKILLS_AND_ATTRIBUTES_FORMAT_VERSION: FormatVersion = 18;
pub const MAX_OLD_CREATURE_STATS_FORMAT_VERSION: FormatVersion = 19;
pub const MAX_LIMITED_SIZE_STRINGS_FORMAT_VERSION: FormatVersion = 22;
pub const MAX_STRING_REF_ID_FORMAT_VERSION: FormatVersion = 23;
pub const MAX_SAVED_GAME_CELL_NAME_AS_REF_ID_FORMAT_VERSION: FormatVersion = 24;
pub const MAX_NAME_IS_REF_ID_ONLY_FORMAT_VERSION: FormatVersion = 25;
pub const MAX_USE_ESM_CELL_ID_FORMAT_VERSION: FormatVersion = 26;
pub const MAX_ACTIVE_SPELL_SLOT_INDEX_FORMAT_VERSION: FormatVersion = 27;
pub const MAX_OLD_COUNT_FORMAT_VERSION: FormatVersion = 30;
pub const MAX_ACTIVE_SPELL_TYPE_VERSION: FormatVersion = 31;
pub const MAX_PLAYER_BEFORE_CELL_DATA_FORMAT_VERSION: FormatVersion = 32;
pub const CURRENT_SAVE_GAME_FORMAT_VERSION: FormatVersion = 34;
pub const MAX_SERIALIZE_EFFECT_REFID_FORMAT_VERSION: FormatVersion = 35;
// Supported save game format versions
pub const MIN_SUPPORTED_SAVE_GAME_FORMAT_VERSION: FormatVersion = 5;
pub const TES3MP_RUST_0_1_SAVE_GAME_FORMAT_VERSION: FormatVersion =
    CURRENT_SAVE_GAME_FORMAT_VERSION;
