use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use strum_macros::Display;

pub const fn esm3_recname_to_bytes(bytes: &[u8; 4]) -> u32 {
    (bytes[0] as u32)
        | ((bytes[1] as u32) << 8)
        | ((bytes[2] as u32) << 16)
        | ((bytes[3] as u32) << 24)
}
impl TryFrom<&str> for RecNameBytes {
    type Error = &'static str;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if s.len() != 4 {
            return Err("Input string must be 4 bytes long.");
        }
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(s.as_bytes());
        let u32_val = esm3_recname_to_bytes(&bytes);
        if let Some(rec_name) = RecNameBytes::from_u32(u32_val) {
            Ok(rec_name)
        } else {
            Err("Invalid record name string.")
        }
    }
}

#[repr(u32)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone, FromPrimitive, Display)]
pub enum RecNameBytes {
    INTERNALPLAYER = 0,
    INTERNALMARKER = 1,

    // format 0 / legacy
    ACTI = esm3_recname_to_bytes(b"ACTI"),
    ALCH = esm3_recname_to_bytes(b"ALCH"),
    APPA = esm3_recname_to_bytes(b"APPA"),
    ARMO = esm3_recname_to_bytes(b"ARMO"),
    BODY = esm3_recname_to_bytes(b"BODY"),
    BOOK = esm3_recname_to_bytes(b"BOOK"),
    BSGN = esm3_recname_to_bytes(b"BSGN"),
    CELL = esm3_recname_to_bytes(b"CELL"),
    CLAS = esm3_recname_to_bytes(b"CLAS"),
    CLOT = esm3_recname_to_bytes(b"CLOT"),
    CNTC = esm3_recname_to_bytes(b"CNTC"),
    CONT = esm3_recname_to_bytes(b"CONT"),
    CREA = esm3_recname_to_bytes(b"CREA"),
    CREC = esm3_recname_to_bytes(b"CREC"),
    DIAL = esm3_recname_to_bytes(b"DIAL"),
    DOOR = esm3_recname_to_bytes(b"DOOR"),
    ENCH = esm3_recname_to_bytes(b"ENCH"),
    FACT = esm3_recname_to_bytes(b"FACT"),
    GLOB = esm3_recname_to_bytes(b"GLOB"),
    GMST = esm3_recname_to_bytes(b"GMST"),
    INFO = esm3_recname_to_bytes(b"INFO"),
    INGR = esm3_recname_to_bytes(b"INGR"),
    LAND = esm3_recname_to_bytes(b"LAND"),
    LEVC = esm3_recname_to_bytes(b"LEVC"),
    LEVI = esm3_recname_to_bytes(b"LEVI"),
    LIGH = esm3_recname_to_bytes(b"LIGH"),
    LOCK = esm3_recname_to_bytes(b"LOCK"),
    LTEX = esm3_recname_to_bytes(b"LTEX"),
    MGEF = esm3_recname_to_bytes(b"MGEF"),
    MISC = esm3_recname_to_bytes(b"MISC"),
    NPC_ = esm3_recname_to_bytes(b"NPC_"),
    NPCC = esm3_recname_to_bytes(b"NPCC"),
    PGRD = esm3_recname_to_bytes(b"PGRD"),
    PROB = esm3_recname_to_bytes(b"PROB"),
    RACE = esm3_recname_to_bytes(b"RACE"),
    REGN = esm3_recname_to_bytes(b"REGN"),
    REPA = esm3_recname_to_bytes(b"REPA"),
    SCPT = esm3_recname_to_bytes(b"SCPT"),
    SKIL = esm3_recname_to_bytes(b"SKIL"),
    SNDG = esm3_recname_to_bytes(b"SNDG"),
    SOUN = esm3_recname_to_bytes(b"SOUN"),
    SPEL = esm3_recname_to_bytes(b"SPEL"),
    SSCR = esm3_recname_to_bytes(b"SSCR"),
    STAT = esm3_recname_to_bytes(b"STAT"),
    WEAP = esm3_recname_to_bytes(b"WEAP"),

    // format 0 - saved games
    SAVE = esm3_recname_to_bytes(b"SAVE"),
    JOUR = esm3_recname_to_bytes(b"JOUR"),
    QUES = esm3_recname_to_bytes(b"QUES"),
    GSCR = esm3_recname_to_bytes(b"GSCR"),
    PLAY = esm3_recname_to_bytes(b"PLAY"),
    CSTA = esm3_recname_to_bytes(b"CSTA"),
    GMAP = esm3_recname_to_bytes(b"GMAP"),
    DIAS = esm3_recname_to_bytes(b"DIAS"),
    WTHR = esm3_recname_to_bytes(b"WTHR"),
    KEYS = esm3_recname_to_bytes(b"KEYS"),
    DYNA = esm3_recname_to_bytes(b"DYNA"),
    ASPL = esm3_recname_to_bytes(b"ASPL"),
    ACTC = esm3_recname_to_bytes(b"ACTC"),
    MPRJ = esm3_recname_to_bytes(b"MPRJ"),
    PROJ = esm3_recname_to_bytes(b"PROJ"),
    DCOU = esm3_recname_to_bytes(b"DCOU"),
    MARK = esm3_recname_to_bytes(b"MARK"),
    ENAB = esm3_recname_to_bytes(b"ENAB"),
    CAM_ = esm3_recname_to_bytes(b"CAM_"),
    STLN = esm3_recname_to_bytes(b"STLN"),
    INPU = esm3_recname_to_bytes(b"INPU"),

    // format 1
    FILT = esm3_recname_to_bytes(b"FILT"),
    DBGP = esm3_recname_to_bytes(b"DBGP"),
    ///< only used in project files
    SELG = esm3_recname_to_bytes(b"SELG"),

    LUAL = esm3_recname_to_bytes(b"LUAL"), // LuaScriptsCfg (only in omwgame or omwaddon)

    // format 16 - Lua scripts in saved games
    LUAM = esm3_recname_to_bytes(b"LUAM"), // LuaManager data

    // format 21 - Random state in saved games.
    RAND = esm3_recname_to_bytes(b"RAND"), // Random state.

    ATTR = esm3_recname_to_bytes(b"ATTR"), // Attribute
}
