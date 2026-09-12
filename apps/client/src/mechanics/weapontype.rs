use std::sync::OnceLock;

use esm_parser_lib::{
    RefId,
    types::{enums::WeaponType, skill::SkillIndex},
};

use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct WeaponDefinitionFlags: u32 {
        const TWO_HANDED = 0x01;
        const HAS_HEALTH = 0x02;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponCategory {
    Melee = 0,
    Ranged = 1,
    Thrown = 2,
    Ammo = 3,
}

pub struct WeaponDefinition {
    pub short_group: String,
    pub long_group: String,
    pub sound_id_down: RefId,
    pub sound_id_up: RefId,
    pub attach_bone: String,
    pub sheathing_bone: String,
    pub skill: RefId,
    pub category: WeaponCategory,
    pub ammo_type: i32,
    pub flags: WeaponDefinitionFlags,
}

impl WeaponDefinition {
    pub fn new(
        short_group: String,
        long_group: String,
        sound_id: &str,
        attach_bone: String,
        sheathing_bone: String,
        skill: RefId,
        category: WeaponCategory,
        ammo_type: i32,
        flags: WeaponDefinitionFlags,
    ) -> Self {
        Self {
            short_group,
            long_group,
            sound_id_down: RefId::from_str(&format!("{} Down", sound_id)),
            sound_id_up: RefId::from_str(&format!("{} Up", sound_id)),
            attach_bone,
            sheathing_bone,
            skill,
            category,
            ammo_type,
            flags,
        }
    }
}
pub fn get_weapon_definition(weapon_type: WeaponType) -> &'static WeaponDefinition {
    static REGISTRY: OnceLock<Vec<WeaponDefinition>> = OnceLock::new();

    let registry = REGISTRY.get_or_init(|| {
        WEAPON_REGISTRY_RAW
            .iter()
            .map(|raw| {
                // Convert WeaponData into WeaponDefinition
                WeaponDefinition::new(
                    raw.short_group.to_string(),
                    raw.long_group.to_string(),
                    raw.sound_id_base,
                    raw.attach_bone.to_string(),
                    raw.sheathing_bone.to_string(),
                    SkillIndex::index_to_refid(raw.skill as i8),
                    raw.category,
                    raw.ammo_type,
                    raw.flags,
                )
            })
            .collect()
    });
    // add 4 since the weapon type starts at -4
    &registry[(weapon_type as i16 + 4) as usize]
}
pub struct WeaponData {
    pub short_group: &'static str,
    pub long_group: &'static str,
    pub sound_id_base: &'static str,
    pub attach_bone: &'static str,
    pub sheathing_bone: &'static str,
    pub skill: SkillIndex, // We'll convert to RefId at runtime
    pub category: WeaponCategory,
    pub ammo_type: i32,
    pub flags: WeaponDefinitionFlags,
}

const WEAPON_REGISTRY_RAW: &[WeaponData] = &[
    // Index 0 (-4): PickProbe
    WeaponData {
        short_group: "1h",
        long_group: "pickprobe",
        sound_id_base: "",
        attach_bone: "",
        sheathing_bone: "",
        skill: SkillIndex::Security,
        category: WeaponCategory::Melee,
        ammo_type: 0,
        flags: WeaponDefinitionFlags::empty(),
    },
    // Index 1 (-3): HandToHand
    WeaponData {
        short_group: "hh",
        long_group: "handtohand",
        sound_id_base: "",
        attach_bone: "",
        sheathing_bone: "",
        skill: SkillIndex::HandToHand,
        category: WeaponCategory::Melee,
        ammo_type: 0,
        flags: WeaponDefinitionFlags::TWO_HANDED,
    },
    // Index 2 (-2): Spell
    WeaponData {
        short_group: "spell",
        long_group: "spellcast",
        sound_id_base: "",
        attach_bone: "",
        sheathing_bone: "",
        skill: SkillIndex::HandToHand,
        category: WeaponCategory::Melee,
        ammo_type: 0,
        flags: WeaponDefinitionFlags::TWO_HANDED,
    },
    // Index 3 (-1): None
    WeaponData {
        short_group: "",
        long_group: "",
        sound_id_base: "",
        attach_bone: "",
        sheathing_bone: "",
        skill: SkillIndex::HandToHand,
        category: WeaponCategory::Melee,
        ammo_type: 0,
        flags: WeaponDefinitionFlags::empty(),
    },
    // Index 4 (0): ShortBladeOneHand
    WeaponData {
        short_group: "1s",
        long_group: "shortbladeonehand",
        sound_id_base: "Item Weapon Shortblade",
        attach_bone: "Weapon Bone",
        sheathing_bone: "Bip01 ShortBladeOneHand",
        skill: SkillIndex::ShortBlade,
        category: WeaponCategory::Melee,
        ammo_type: 0,
        flags: WeaponDefinitionFlags::HAS_HEALTH,
    },
    // Index 5 (1): LongBladeOneHand
    WeaponData {
        short_group: "1h",
        long_group: "weapononehand",
        sound_id_base: "Item Weapon Longblade",
        attach_bone: "Weapon Bone",
        sheathing_bone: "Bip01 LongBladeOneHand",
        skill: SkillIndex::LongBlade,
        category: WeaponCategory::Melee,
        ammo_type: 0,
        flags: WeaponDefinitionFlags::HAS_HEALTH,
    },
    // Index 6 (2): LongBladeTwoClose
    WeaponData {
        short_group: "2c",
        long_group: "weapontwohand",
        sound_id_base: "Item Weapon Longblade",
        attach_bone: "Weapon Bone",
        sheathing_bone: "Bip01 LongBladeTwoClose",
        skill: SkillIndex::LongBlade,
        category: WeaponCategory::Melee,
        ammo_type: 0,
        flags: HAS_HEALTH_TWO_HANDED,
    },
    // Index 7 (3): BluntOneHand
    WeaponData {
        short_group: "1b",
        long_group: "bluntonehand",
        sound_id_base: "Item Weapon Blunt",
        attach_bone: "Weapon Bone",
        sheathing_bone: "Bip01 BluntOneHand",
        skill: SkillIndex::BluntWeapon,
        category: WeaponCategory::Melee,
        ammo_type: 0,
        flags: WeaponDefinitionFlags::HAS_HEALTH,
    },
    // Index 8 (4): BluntTwoClose
    WeaponData {
        short_group: "2b",
        long_group: "blunttwohand",
        sound_id_base: "Item Weapon Blunt",
        attach_bone: "Weapon Bone",
        sheathing_bone: "Bip01 BluntTwoClose",
        skill: SkillIndex::BluntWeapon,
        category: WeaponCategory::Melee,
        ammo_type: 0,
        flags: HAS_HEALTH_TWO_HANDED,
    },
    // Index 9 (5): BluntTwoWide
    WeaponData {
        short_group: "2w",
        long_group: "weapontwowide",
        sound_id_base: "Item Weapon Blunt",
        attach_bone: "Weapon Bone",
        sheathing_bone: "Bip01 BluntTwoWide",
        skill: SkillIndex::BluntWeapon,
        category: WeaponCategory::Melee,
        ammo_type: 0,
        flags: HAS_HEALTH_TWO_HANDED,
    },
    // Index 10 (6): SpearTwoWide
    WeaponData {
        short_group: "2w",
        long_group: "weapontwowide",
        sound_id_base: "Item Weapon Spear",
        attach_bone: "Weapon Bone",
        sheathing_bone: "Bip01 SpearTwoWide",
        skill: SkillIndex::Spear,
        category: WeaponCategory::Melee,
        ammo_type: 0,
        flags: HAS_HEALTH_TWO_HANDED,
    },
    // Index 11 (7): AxeOneHand
    WeaponData {
        short_group: "1b",
        long_group: "bluntonehand",
        sound_id_base: "Item Weapon Blunt",
        attach_bone: "Weapon Bone",
        sheathing_bone: "Bip01 LongBladeOneHand",
        skill: SkillIndex::Axe,
        category: WeaponCategory::Melee,
        ammo_type: 0,
        flags: WeaponDefinitionFlags::HAS_HEALTH,
    },
    // Index 12 (8): AxeTwoHand
    WeaponData {
        short_group: "2b",
        long_group: "blunttwohand",
        sound_id_base: "Item Weapon Blunt",
        attach_bone: "Weapon Bone",
        sheathing_bone: "Bip01 AxeTwoClose",
        skill: SkillIndex::Axe,
        category: WeaponCategory::Melee,
        ammo_type: 0,
        flags: HAS_HEALTH_TWO_HANDED,
    },
    // Index 13 (9): MarksmanBow
    WeaponData {
        short_group: "bow",
        long_group: "bowandarrow",
        sound_id_base: "Item Weapon Bow",
        attach_bone: "Weapon Bone Left",
        sheathing_bone: "Bip01 MarksmanBow",
        skill: SkillIndex::Marksman,
        category: WeaponCategory::Ranged,
        ammo_type: 0,
        flags: HAS_HEALTH_TWO_HANDED,
    },
    // Index 14 (10): MarksmanCrossbow
    WeaponData {
        short_group: "crossbow",
        long_group: "crossbow",
        sound_id_base: "Item Weapon Crossbow",
        attach_bone: "Weapon Bone",
        sheathing_bone: "Bip01 MarksmanCrossbow",
        skill: SkillIndex::Marksman,
        category: WeaponCategory::Ranged,
        ammo_type: 0,
        flags: HAS_HEALTH_TWO_HANDED,
    },
    // Index 15 (11): MarksmanThrown
    WeaponData {
        short_group: "1t",
        long_group: "throwweapon",
        sound_id_base: "Item Weapon Blunt",
        attach_bone: "Weapon Bone",
        sheathing_bone: "Bip01 MarksmanThrown",
        skill: SkillIndex::Marksman,
        category: WeaponCategory::Thrown,
        ammo_type: 0,
        flags: WeaponDefinitionFlags::empty(),
    },
    // Index 16 (12): Arrow
    WeaponData {
        short_group: "",
        long_group: "",
        sound_id_base: "Item Ammo",
        attach_bone: "Bip01 Arrow",
        sheathing_bone: "",
        skill: SkillIndex::Marksman,
        category: WeaponCategory::Ammo,
        ammo_type: 0,
        flags: WeaponDefinitionFlags::empty(),
    },
    // Index 17 (13): Bolt
    WeaponData {
        short_group: "",
        long_group: "",
        sound_id_base: "Item Ammo",
        attach_bone: "ArrowBone",
        sheathing_bone: "",
        skill: SkillIndex::Marksman,
        category: WeaponCategory::Ammo,
        ammo_type: 0,
        flags: WeaponDefinitionFlags::empty(),
    },
];
const HAS_HEALTH_TWO_HANDED: WeaponDefinitionFlags =
    WeaponDefinitionFlags::HAS_HEALTH.union(WeaponDefinitionFlags::TWO_HANDED);
