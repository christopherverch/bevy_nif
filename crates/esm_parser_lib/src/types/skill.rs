use crate::{
    EsmError, EsmReader, Load, RefId, StringRefId,
    esm_record::EsmRecord,
    name4,
    types::{enums::Specialization, flags::ObjectFlags},
};
use esm_parser_macros::LoadSave;
#[derive(Default, Debug)]
pub struct Skill {
    pub record_flags: ObjectFlags,
    pub id: RefId,
    pub index: SkillIndex,
    pub data: SkillData,

    // These are empty strings if not loaded
    pub description: String,
    pub name: String,
    pub icon: String,

    pub werewolf_value: f32,

    // This is set up during the final setup, not during loading
    pub school: Option<MagicSchool>,
}
impl EsmRecord for Skill {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: u32 = name4!("SKIL").to_u32();
}
impl Skill {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Skill, EsmError> {
        // Skills are generally not deleted in vanilla ESMs
        *is_deleted = false;
        let record_flags = reader.get_record_flags();
        let mut skill = Skill {
            record_flags,
            ..Default::default()
        };

        let mut has_index = false;
        let mut has_data = false;
        let mut skill_index = SkillIndex::HandToHand;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;
            match sub_name.as_bytes() {
                b"INDX" => {
                    skill_index = reader.read_typed_header_load()?;
                    has_index = true;
                }
                b"SKDT" => {
                    skill.data = reader.read_typed_header_load()?;
                    has_data = true;
                }
                b"DESC" => skill.description = reader.read_header_decode_string()?,
                _ => {
                    return Err(reader.fail(&format!(
                        "Unknown subrecord: {}",
                        String::from_utf8_lossy(sub_name.as_bytes())
                    )));
                }
            }
        }

        if !has_index {
            return Err(reader.fail("Missing INDX subrecord for Skill"));
        }
        if !has_data {
            return Err(reader.fail("Missing SKDT subrecord for Skill"));
        }
        skill.index = skill_index;
        skill.id = SkillIndex::index_to_refid(skill.index as i8);

        Ok(skill)
    }
}

#[derive(LoadSave, Debug)]
pub struct SkillData {
    pub attribute: i32,
    /// 0 - Combat, 1 - Magic, 2 - Stealth
    pub specialization: Specialization,

    /// How much skill improves through use. Meaning of each field depends on what skill
    /// this is. See SkillUsage above
    pub use_value: [f32; 4],
}
impl Default for SkillData {
    fn default() -> Self {
        Self {
            attribute: 0,
            specialization: Specialization::Combat,
            use_value: [1.0; 4],
        }
    }
}
pub struct SkillUsage;

impl SkillUsage {
    // These are shared by multiple skills
    pub const ARMOR_HIT_BY_OPPONENT: usize = 0;
    pub const BLOCK_SUCCESS: usize = 0;
    pub const SPELLCAST_SUCCESS: usize = 0;
    pub const WEAPON_SUCCESSFUL_HIT: usize = 0;

    // Skill-specific use types
    pub const ALCHEMY_CREATE_POTION: usize = 0;
    pub const ALCHEMY_USE_INGREDIENT: usize = 1;

    pub const ENCHANT_RECHARGE: usize = 0;
    pub const ENCHANT_USE_MAGIC_ITEM: usize = 1;
    pub const ENCHANT_CREATE_MAGIC_ITEM: usize = 2;
    pub const ENCHANT_CAST_ON_STRIKE: usize = 3;

    pub const ACROBATICS_JUMP: usize = 0;
    pub const ACROBATICS_FALL: usize = 1;

    pub const MERCANTILE_SUCCESS: usize = 0;
    /// Note: This is bugged in vanilla and is not actually in use.
    pub const MERCANTILE_BRIBE: usize = 1;

    pub const SECURITY_DISARM_TRAP: usize = 0;
    pub const SECURITY_PICK_LOCK: usize = 1;

    pub const SNEAK_AVOID_NOTICE: usize = 0;
    pub const SNEAK_PICK_POCKET: usize = 1;

    pub const SPEECHCRAFT_SUCCESS: usize = 0;
    pub const SPEECHCRAFT_FAIL: usize = 1;

    pub const ARMORER_REPAIR: usize = 0;

    pub const ATHLETICS_RUN_ONE_SECOND: usize = 0;
    pub const ATHLETICS_SWIM_ONE_SECOND: usize = 1;
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SkillIndex {
    #[default]
    Block = 0,
    Armorer = 1,
    MediumArmor = 2,
    HeavyArmor = 3,
    BluntWeapon = 4,
    LongBlade = 5,
    Axe = 6,
    Spear = 7,
    Athletics = 8,
    Enchant = 9,
    Destruction = 10,
    Alteration = 11,
    Illusion = 12,
    Conjuration = 13,
    Mysticism = 14,
    Restoration = 15,
    Alchemy = 16,
    Unarmored = 17,
    Security = 18,
    Sneak = 19,
    Acrobatics = 20,
    LightArmor = 21,
    ShortBlade = 22,
    Marksman = 23,
    Mercantile = 24,
    Speechcraft = 25,
    HandToHand = 26,
}
impl Load for SkillIndex {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError>
    where
        Self: Sized,
    {
        let raw_index: i32 = reader.load()?;

        if raw_index < 0 || raw_index >= SkillIndex::NUM_SKILLS as i32 {
            return Err(reader.fail(&format!("Invalid Skill Index: {}", raw_index)));
        }
        let skill_index = raw_index.try_into().unwrap();
        Ok(skill_index)
    }
}
impl TryFrom<i32> for SkillIndex {
    type Error = ();

    #[inline]
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        // Benchmarks to a single CMP and JMP instruction
        if value >= 0 && value < Self::NUM_SKILLS as i32 {
            // SAFETY: SkillIndex is repr(i32) and we have verified the
            // value is within the valid range of the enum variants.
            Ok(unsafe { std::mem::transmute(value) })
        } else {
            Err(())
        }
    }
}

impl SkillIndex {
    pub const NUM_SKILLS: usize = 27;
    const SKILL_IDS: [RefId; Self::NUM_SKILLS] = [
        RefId::SizedString(StringRefId("Block")),
        RefId::SizedString(StringRefId("Armorer")),
        RefId::SizedString(StringRefId("MediumArmor")),
        RefId::SizedString(StringRefId("HeavyArmor")),
        RefId::SizedString(StringRefId("BluntWeapon")),
        RefId::SizedString(StringRefId("LongBlade")),
        RefId::SizedString(StringRefId("Axe")),
        RefId::SizedString(StringRefId("Spear")),
        RefId::SizedString(StringRefId("Athletics")),
        RefId::SizedString(StringRefId("Enchant")),
        RefId::SizedString(StringRefId("Destruction")),
        RefId::SizedString(StringRefId("Alteration")),
        RefId::SizedString(StringRefId("Illusion")),
        RefId::SizedString(StringRefId("Conjuration")),
        RefId::SizedString(StringRefId("Mysticism")),
        RefId::SizedString(StringRefId("Restoration")),
        RefId::SizedString(StringRefId("Alchemy")),
        RefId::SizedString(StringRefId("Unarmored")),
        RefId::SizedString(StringRefId("Security")),
        RefId::SizedString(StringRefId("Sneak")),
        RefId::SizedString(StringRefId("Acrobatics")),
        RefId::SizedString(StringRefId("LightArmor")),
        RefId::SizedString(StringRefId("ShortBlade")),
        RefId::SizedString(StringRefId("Marksman")),
        RefId::SizedString(StringRefId("Mercantile")),
        RefId::SizedString(StringRefId("Speechcraft")),
        RefId::SizedString(StringRefId("HandToHand")),
    ];
    pub fn index_to_refid(index: i8) -> RefId {
        if (index as i32) < 0 || (index as i32) >= Self::NUM_SKILLS as i32 {
            return RefId::default();
        }
        Self::SKILL_IDS.get(index as usize).unwrap().clone()
    }

    pub fn refid_to_skillindex(id: &RefId) -> Option<usize> {
        for (index, skill_id) in Self::SKILL_IDS.iter().enumerate() {
            if skill_id == id {
                return Some(index);
            }
        }
        None
    }
    pub fn to_refid(self) -> RefId {
        Self::SKILL_IDS.get(self as usize).unwrap().clone()
    }
}
#[derive(Debug, Default)]
pub struct MagicSchool {
    pub area_sound: RefId,
    pub bolt_sound: RefId,
    pub cast_sound: RefId,
    pub failure_sound: RefId,
    pub hit_sound: RefId,
    pub name: String,
    pub auto_calc_max: i32,
}

impl MagicSchool {
    pub const NUM_SCHOOLS: usize = 6;

    const SCHOOL_SKILLS: [SkillIndex; Self::NUM_SCHOOLS] = [
        SkillIndex::Alteration,  // 0
        SkillIndex::Conjuration, // 1
        SkillIndex::Destruction, // 2
        SkillIndex::Illusion,    // 3
        SkillIndex::Mysticism,   // 4
        SkillIndex::Restoration, // 5
    ];

    pub fn index_to_skill_id(index: i32) -> RefId {
        if index < 0 || index >= Self::NUM_SCHOOLS as i32 {
            return RefId::default();
        }
        let skill_enum = Self::SCHOOL_SKILLS[index as usize];
        // Now convert that specific SkillIndex to its RefId
        SkillIndex::index_to_refid(skill_enum as i8)
    }
}
