use std::{
    collections::HashMap,
    io::{Read, Seek},
    sync::LazyLock,
};

use crate::{
    EsmError, EsmReader, RefId, StringRefId,
    common_consts::NUM_MAGIC_EFFECTS,
    esm_record::EsmRecord,
    format_version_defs::DEFAULT_FORMAT_VERSION,
    name4,
    types::{
        flags::{MagicEffectFlags, ObjectFlags},
        skill::MagicSchool,
    },
};

#[derive(Default, Debug)]
pub struct MagicEffect {
    pub record_flags: ObjectFlags,
    pub id: RefId,
    pub magic_effect_data: MagicEffectData,
    // Textures
    pub icon: String,
    pub particle: String,
    // Static
    pub casting_vfx: RefId,
    pub hit_vfx: RefId,
    pub area_vfx: RefId,
    // Weapon
    pub bolt_vfx: RefId,
    // Sounds
    pub cast_sound: RefId,
    pub bolt_sound: RefId,
    pub hit_sound: RefId,
    pub area_sound: RefId,

    pub description: String,
    /// Effect GMST (not part of the MGEF definition)
    pub name: String,
}
impl EsmRecord for MagicEffect {
    fn id(&self) -> &RefId {
        &self.id
    }
}
#[derive(Default, Debug)]
pub struct MagicEffectData {
    pub school: RefId,
    pub base_cost: f32,
    pub flags: MagicEffectFlags,
    /// Rgb
    pub glow_color: [i32; 3],
    pub _unknown1: f32,
    pub projectile_speed: f32,
    pub _unknown2: f32,
}
impl MagicEffect {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<MagicEffect, EsmError> {
        *is_deleted = false;
        let record_flags: ObjectFlags = reader.get_record_flags();
        // INDX: The effect ID index
        let index: i32 = reader.read_typed_named_header(name4!("INDX"))?;
        let id = Self::index_to_refid(index);
        // MEDT: The main data block
        reader.get_or_read_validated_subname(name4!("MEDT"))?;
        reader.read_sub_header()?;

        let school_index: i32 = reader.load()?;
        let school = MagicSchool::index_to_skill_id(school_index);
        let base_cost = reader.load()?;
        let flags = reader.load()?;
        let glow_color = reader.load()?;
        let unknown1 = reader.load()?;
        let projectile_speed = reader.load()?;
        let unknown2 = reader.load()?;

        let mut magic_effect_data = MagicEffectData {
            school,
            base_cost,
            flags,
            glow_color,
            _unknown1: unknown1,
            projectile_speed,
            _unknown2: unknown2,
        };
        if reader.header.format_version == DEFAULT_FORMAT_VERSION {
            // Don't allow mods to change fixed flags in the legacy format
            magic_effect_data.flags &= MagicEffectFlags::ALLOW_SPELLMAKING
                | MagicEffectFlags::ALLOW_ENCHANTING
                | MagicEffectFlags::NEGATIVE_LIGHTING;
            if index >= 0 && index < NUM_MAGIC_EFFECTS {
                magic_effect_data.flags |= Self::HARDCODED_FLAGS[index as usize];
            }
        }
        let mut magic_effect = MagicEffect {
            id,
            record_flags,
            magic_effect_data,
            ..Default::default()
        };
        while reader.has_more_subs() {
            let current_pos = reader.file.stream_position().unwrap();
            let mut probe = [0u8; 4];
            reader.file.read_exact(&mut probe).unwrap();
            reader
                .file
                .seek(std::io::SeekFrom::Start(current_pos))
                .unwrap(); // seek back

            let sub_name = reader.get_or_read_sub_name()?;
            match sub_name.as_bytes() {
                b"ITEX" => magic_effect.icon = reader.read_header_decode_string()?,
                b"PTEX" => magic_effect.particle = reader.read_header_decode_string()?,
                b"BSND" => magic_effect.bolt_sound = reader.read_refid()?,
                b"CSND" => magic_effect.cast_sound = reader.read_refid()?,
                b"HSND" => magic_effect.hit_sound = reader.read_refid()?,
                b"ASND" => magic_effect.area_sound = reader.read_refid()?,
                b"CVFX" => magic_effect.casting_vfx = reader.read_refid()?,
                b"BVFX" => magic_effect.bolt_vfx = reader.read_refid()?,
                b"HVFX" => magic_effect.hit_vfx = reader.read_refid()?,
                b"AVFX" => magic_effect.area_vfx = reader.read_refid()?,
                b"DESC" => magic_effect.description = reader.read_header_decode_string()?,
                _ => {
                    // If your reader doesn't skip automatically, uncomment:
                    // reader.skip_sub()?;
                    return Err(reader.fail(&format!(
                        "Unknown MagicEffect subrecord: {}",
                        sub_name.to_str()
                    )));
                }
            }
        }
        Ok(magic_effect)
    }
    pub fn index_to_refid(index: i32) -> RefId {
        if index < 0 || index >= NUM_MAGIC_EFFECTS {
            return RefId::default();
        }
        Self::MAGIC_EFFECT_IDS.get(index as usize).unwrap().clone()
    }
    pub fn index_to_gmst_str(index: i32) -> &'static str {
        if index < 0 || index >= NUM_MAGIC_EFFECTS {
            return "";
        }
        Self::GMST_EFFECT_IDS.get(index as usize).unwrap()
    }

    const HARDCODED_FLAGS: [MagicEffectFlags; NUM_MAGIC_EFFECTS as usize] = [
        MagicEffectFlags::from_bits_retain(0x11c8),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c8),
        MagicEffectFlags::from_bits_retain(0x11e0),
        MagicEffectFlags::from_bits_retain(0x11e0),
        MagicEffectFlags::from_bits_retain(0x11e0),
        MagicEffectFlags::from_bits_retain(0x11e0),
        MagicEffectFlags::from_bits_retain(0x11d0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11e0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11184),
        MagicEffectFlags::from_bits_retain(0x11184),
        MagicEffectFlags::from_bits_retain(0x1f0),
        MagicEffectFlags::from_bits_retain(0x1f0),
        MagicEffectFlags::from_bits_retain(0x1f0),
        MagicEffectFlags::from_bits_retain(0x11d2),
        MagicEffectFlags::from_bits_retain(0x11f0),
        MagicEffectFlags::from_bits_retain(0x11d0),
        MagicEffectFlags::from_bits_retain(0x11d0),
        MagicEffectFlags::from_bits_retain(0x11d1),
        MagicEffectFlags::from_bits_retain(0x1d2),
        MagicEffectFlags::from_bits_retain(0x1f0),
        MagicEffectFlags::from_bits_retain(0x1d0),
        MagicEffectFlags::from_bits_retain(0x1d0),
        MagicEffectFlags::from_bits_retain(0x1d1),
        MagicEffectFlags::from_bits_retain(0x1f0),
        MagicEffectFlags::from_bits_retain(0x11d0),
        MagicEffectFlags::from_bits_retain(0x11d0),
        MagicEffectFlags::from_bits_retain(0x11d0),
        MagicEffectFlags::from_bits_retain(0x11d0),
        MagicEffectFlags::from_bits_retain(0x11d0),
        MagicEffectFlags::from_bits_retain(0x11d0),
        MagicEffectFlags::from_bits_retain(0x11d0),
        MagicEffectFlags::from_bits_retain(0x11d0),
        MagicEffectFlags::from_bits_retain(0x11d0),
        MagicEffectFlags::from_bits_retain(0x1d0),
        MagicEffectFlags::from_bits_retain(0x1d0),
        MagicEffectFlags::from_bits_retain(0x11c8),
        MagicEffectFlags::from_bits_retain(0x31c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x1180),
        MagicEffectFlags::from_bits_retain(0x11d8),
        MagicEffectFlags::from_bits_retain(0x11d8),
        MagicEffectFlags::from_bits_retain(0x11d0),
        MagicEffectFlags::from_bits_retain(0x11d0),
        MagicEffectFlags::from_bits_retain(0x11180),
        MagicEffectFlags::from_bits_retain(0x11180),
        MagicEffectFlags::from_bits_retain(0x11180),
        MagicEffectFlags::from_bits_retain(0x11180),
        MagicEffectFlags::from_bits_retain(0x11180),
        MagicEffectFlags::from_bits_retain(0x11180),
        MagicEffectFlags::from_bits_retain(0x11180),
        MagicEffectFlags::from_bits_retain(0x11180),
        MagicEffectFlags::from_bits_retain(0x11c4),
        MagicEffectFlags::from_bits_retain(0x111b8),
        MagicEffectFlags::from_bits_retain(0x1040),
        MagicEffectFlags::from_bits_retain(0x104c),
        MagicEffectFlags::from_bits_retain(0x104c),
        MagicEffectFlags::from_bits_retain(0x104c),
        MagicEffectFlags::from_bits_retain(0x104c),
        MagicEffectFlags::from_bits_retain(0x1040),
        MagicEffectFlags::from_bits_retain(0x1040),
        MagicEffectFlags::from_bits_retain(0x1040),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x1cc),
        MagicEffectFlags::from_bits_retain(0x1cc),
        MagicEffectFlags::from_bits_retain(0x1cc),
        MagicEffectFlags::from_bits_retain(0x1cc),
        MagicEffectFlags::from_bits_retain(0x1cc),
        MagicEffectFlags::from_bits_retain(0x1c2),
        MagicEffectFlags::from_bits_retain(0x1c0),
        MagicEffectFlags::from_bits_retain(0x1c0),
        MagicEffectFlags::from_bits_retain(0x1c0),
        MagicEffectFlags::from_bits_retain(0x1c1),
        MagicEffectFlags::from_bits_retain(0x11c2),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c1),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x21192),
        MagicEffectFlags::from_bits_retain(0x20190),
        MagicEffectFlags::from_bits_retain(0x20190),
        MagicEffectFlags::from_bits_retain(0x20190),
        MagicEffectFlags::from_bits_retain(0x21191),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x1c0),
        MagicEffectFlags::from_bits_retain(0x11190),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x9048),
        MagicEffectFlags::from_bits_retain(0x11c0),
        MagicEffectFlags::from_bits_retain(0x1180),
        MagicEffectFlags::from_bits_retain(0x1180),
        MagicEffectFlags::from_bits_retain(0x5048),
        MagicEffectFlags::from_bits_retain(0x5048),
        MagicEffectFlags::from_bits_retain(0x5048),
        MagicEffectFlags::from_bits_retain(0x5048),
        MagicEffectFlags::from_bits_retain(0x5048),
        MagicEffectFlags::from_bits_retain(0x5048),
        MagicEffectFlags::from_bits_retain(0x1188),
        MagicEffectFlags::from_bits_retain(0x5048),
        MagicEffectFlags::from_bits_retain(0x5048),
        MagicEffectFlags::from_bits_retain(0x5048),
        MagicEffectFlags::from_bits_retain(0x5048),
        MagicEffectFlags::from_bits_retain(0x5048),
        MagicEffectFlags::from_bits_retain(0x1048),
        MagicEffectFlags::from_bits_retain(0x104c),
        MagicEffectFlags::from_bits_retain(0x1048),
        MagicEffectFlags::from_bits_retain(0x40),
        MagicEffectFlags::from_bits_retain(0x11c8),
        MagicEffectFlags::from_bits_retain(0x1048),
        MagicEffectFlags::from_bits_retain(0x1048),
        MagicEffectFlags::from_bits_retain(0x1048),
        MagicEffectFlags::from_bits_retain(0x1048),
        MagicEffectFlags::from_bits_retain(0x1048),
        MagicEffectFlags::from_bits_retain(0x1048),
    ];
    pub const MAGIC_EFFECT_IDS: &[RefId] = &[
        Self::WATERBREATHING,
        Self::SWIFTSWIM,
        Self::WATERWALKING,
        Self::SHIELD,
        Self::FIRESHIELD,
        Self::LIGHTNINGSHIELD,
        Self::FROSTSHIELD,
        Self::BURDEN,
        Self::FEATHER,
        Self::JUMP,
        Self::LEVITATE,
        Self::SLOWFALL,
        Self::LOCK,
        Self::OPEN,
        Self::FIREDAMAGE,
        Self::SHOCKDAMAGE,
        Self::FROSTDAMAGE,
        Self::DRAINATTRIBUTE,
        Self::DRAINHEALTH,
        Self::DRAINMAGICKA,
        Self::DRAINFATIGUE,
        Self::DRAINSKILL,
        Self::DAMAGEATTRIBUTE,
        Self::DAMAGEHEALTH,
        Self::DAMAGEMAGICKA,
        Self::DAMAGEFATIGUE,
        Self::DAMAGESKILL,
        Self::POISON,
        Self::WEAKNESSTOFIRE,
        Self::WEAKNESSTOFROST,
        Self::WEAKNESSTOSHOCK,
        Self::WEAKNESSTOMAGICKA,
        Self::WEAKNESSTOCOMMONDISEASE,
        Self::WEAKNESSTOBLIGHTDISEASE,
        Self::WEAKNESSTOCORPRUSDISEASE,
        Self::WEAKNESSTOPOISON,
        Self::WEAKNESSTONORMALWEAPONS,
        Self::DISINTEGRATEWEAPON,
        Self::DISINTEGRATEARMOR,
        Self::INVISIBILITY,
        Self::CHAMELEON,
        Self::LIGHT,
        Self::SANCTUARY,
        Self::NIGHTEYE,
        Self::CHARM,
        Self::PARALYZE,
        Self::SILENCE,
        Self::BLIND,
        Self::SOUND,
        Self::CALMHUMANOID,
        Self::CALMCREATURE,
        Self::FRENZYHUMANOID,
        Self::FRENZYCREATURE,
        Self::DEMORALIZEHUMANOID,
        Self::DEMORALIZECREATURE,
        Self::RALLYHUMANOID,
        Self::RALLYCREATURE,
        Self::DISPEL,
        Self::SOULTRAP,
        Self::TELEKINESIS,
        Self::MARK,
        Self::RECALL,
        Self::DIVINEINTERVENTION,
        Self::ALMSIVIINTERVENTION,
        Self::DETECTANIMAL,
        Self::DETECTENCHANTMENT,
        Self::DETECTKEY,
        Self::SPELLABSORPTION,
        Self::REFLECT,
        Self::CURECOMMONDISEASE,
        Self::CUREBLIGHTDISEASE,
        Self::CURECORPRUSDISEASE,
        Self::CUREPOISON,
        Self::CUREPARALYZATION,
        Self::RESTOREATTRIBUTE,
        Self::RESTOREHEALTH,
        Self::RESTOREMAGICKA,
        Self::RESTOREFATIGUE,
        Self::RESTORESKILL,
        Self::FORTIFYATTRIBUTE,
        Self::FORTIFYHEALTH,
        Self::FORTIFYMAGICKA,
        Self::FORTIFYFATIGUE,
        Self::FORTIFYSKILL,
        Self::FORTIFYMAXIMUMMAGICKA,
        Self::ABSORBATTRIBUTE,
        Self::ABSORBHEALTH,
        Self::ABSORBMAGICKA,
        Self::ABSORBFATIGUE,
        Self::ABSORBSKILL,
        Self::RESISTFIRE,
        Self::RESISTFROST,
        Self::RESISTSHOCK,
        Self::RESISTMAGICKA,
        Self::RESISTCOMMONDISEASE,
        Self::RESISTBLIGHTDISEASE,
        Self::RESISTCORPRUSDISEASE,
        Self::RESISTPOISON,
        Self::RESISTNORMALWEAPONS,
        Self::RESISTPARALYSIS,
        Self::REMOVECURSE,
        Self::TURNUNDEAD,
        Self::SUMMONSCAMP,
        Self::SUMMONCLANNFEAR,
        Self::SUMMONDAEDROTH,
        Self::SUMMONDREMORA,
        Self::SUMMONANCESTRALGHOST,
        Self::SUMMONSKELETALMINION,
        Self::SUMMONBONEWALKER,
        Self::SUMMONGREATERBONEWALKER,
        Self::SUMMONBONELORD,
        Self::SUMMONWINGEDTWILIGHT,
        Self::SUMMONHUNGER,
        Self::SUMMONGOLDENSAINT,
        Self::SUMMONFLAMEATRONACH,
        Self::SUMMONFROSTATRONACH,
        Self::SUMMONSTORMATRONACH,
        Self::FORTIFYATTACK,
        Self::COMMANDCREATURE,
        Self::COMMANDHUMANOID,
        Self::BOUNDDAGGER,
        Self::BOUNDLONGSWORD,
        Self::BOUNDMACE,
        Self::BOUNDBATTLEAXE,
        Self::BOUNDSPEAR,
        Self::BOUNDLONGBOW,
        Self::EXTRASPELL,
        Self::BOUNDCUIRASS,
        Self::BOUNDHELM,
        Self::BOUNDBOOTS,
        Self::BOUNDSHIELD,
        Self::BOUNDGLOVES,
        Self::CORPRUS,
        Self::VAMPIRISM,
        Self::SUMMONCENTURIONSPHERE,
        Self::SUNDAMAGE,
        Self::STUNTEDMAGICKA,
        Self::SUMMONFABRICANT,
        Self::SUMMONWOLF,
        Self::SUMMONBEAR,
        Self::SUMMONBONEWOLF,
        Self::SUMMONCREATURE04,
        Self::SUMMONCREATURE05,
    ];
    pub const GMST_EFFECT_IDS: [&str; NUM_MAGIC_EFFECTS as usize] = [
        "sEffectWaterBreathing",
        "sEffectSwiftSwim",
        "sEffectWaterWalking",
        "sEffectShield",
        "sEffectFireShield",
        "sEffectLightningShield",
        "sEffectFrostShield",
        "sEffectBurden",
        "sEffectFeather",
        "sEffectJump",
        "sEffectLevitate",
        "sEffectSlowFall",
        "sEffectLock",
        "sEffectOpen",
        "sEffectFireDamage",
        "sEffectShockDamage",
        "sEffectFrostDamage",
        "sEffectDrainAttribute",
        "sEffectDrainHealth",
        "sEffectDrainSpellpoints",
        "sEffectDrainFatigue",
        "sEffectDrainSkill",
        "sEffectDamageAttribute",
        "sEffectDamageHealth",
        "sEffectDamageMagicka",
        "sEffectDamageFatigue",
        "sEffectDamageSkill",
        "sEffectPoison",
        "sEffectWeaknesstoFire",
        "sEffectWeaknesstoFrost",
        "sEffectWeaknesstoShock",
        "sEffectWeaknesstoMagicka",
        "sEffectWeaknesstoCommonDisease",
        "sEffectWeaknesstoBlightDisease",
        "sEffectWeaknesstoCorprusDisease",
        "sEffectWeaknesstoPoison",
        "sEffectWeaknesstoNormalWeapons",
        "sEffectDisintegrateWeapon",
        "sEffectDisintegrateArmor",
        "sEffectInvisibility",
        "sEffectChameleon",
        "sEffectLight",
        "sEffectSanctuary",
        "sEffectNightEye",
        "sEffectCharm",
        "sEffectParalyze",
        "sEffectSilence",
        "sEffectBlind",
        "sEffectSound",
        "sEffectCalmHumanoid",
        "sEffectCalmCreature",
        "sEffectFrenzyHumanoid",
        "sEffectFrenzyCreature",
        "sEffectDemoralizeHumanoid",
        "sEffectDemoralizeCreature",
        "sEffectRallyHumanoid",
        "sEffectRallyCreature",
        "sEffectDispel",
        "sEffectSoultrap",
        "sEffectTelekinesis",
        "sEffectMark",
        "sEffectRecall",
        "sEffectDivineIntervention",
        "sEffectAlmsiviIntervention",
        "sEffectDetectAnimal",
        "sEffectDetectEnchantment",
        "sEffectDetectKey",
        "sEffectSpellAbsorption",
        "sEffectReflect",
        "sEffectCureCommonDisease",
        "sEffectCureBlightDisease",
        "sEffectCureCorprusDisease",
        "sEffectCurePoison",
        "sEffectCureParalyzation",
        "sEffectRestoreAttribute",
        "sEffectRestoreHealth",
        "sEffectRestoreSpellPoints",
        "sEffectRestoreFatigue",
        "sEffectRestoreSkill",
        "sEffectFortifyAttribute",
        "sEffectFortifyHealth",
        "sEffectFortifySpellpoints",
        "sEffectFortifyFatigue",
        "sEffectFortifySkill",
        "sEffectFortifyMagickaMultiplier",
        "sEffectAbsorbAttribute",
        "sEffectAbsorbHealth",
        "sEffectAbsorbSpellPoints",
        "sEffectAbsorbFatigue",
        "sEffectAbsorbSkill",
        "sEffectResistFire",
        "sEffectResistFrost",
        "sEffectResistShock",
        "sEffectResistMagicka",
        "sEffectResistCommonDisease",
        "sEffectResistBlightDisease",
        "sEffectResistCorprusDisease",
        "sEffectResistPoison",
        "sEffectResistNormalWeapons",
        "sEffectResistParalysis",
        "sEffectRemoveCurse",
        "sEffectTurnUndead",
        "sEffectSummonScamp",
        "sEffectSummonClannfear",
        "sEffectSummonDaedroth",
        "sEffectSummonDremora",
        "sEffectSummonAncestralGhost",
        "sEffectSummonSkeletalMinion",
        "sEffectSummonLeastBonewalker",
        "sEffectSummonGreaterBonewalker",
        "sEffectSummonBonelord",
        "sEffectSummonWingedTwilight",
        "sEffectSummonHunger",
        "sEffectSummonGoldenSaint",
        "sEffectSummonFlameAtronach",
        "sEffectSummonFrostAtronach",
        "sEffectSummonStormAtronach",
        "sEffectFortifyAttackBonus",
        "sEffectCommandCreatures",
        "sEffectCommandHumanoids",
        "sEffectBoundDagger",
        "sEffectBoundLongsword",
        "sEffectBoundMace",
        "sEffectBoundBattleAxe",
        "sEffectBoundSpear",
        "sEffectBoundLongbow",
        "sEffectExtraSpell",
        "sEffectBoundCuirass",
        "sEffectBoundHelm",
        "sEffectBoundBoots",
        "sEffectBoundShield",
        "sEffectBoundGloves",
        // NB this typo. (bethesda made it)
        "sEffectCorpus",
        "sEffectVampirism",
        "sEffectSummonCenturionSphere",
        "sEffectSunDamage",
        "sEffectStuntedMagicka",
        // tribunal
        "sEffectSummonFabricant",
        // bloodmoon
        "sEffectSummonCreature01",
        "sEffectSummonCreature02",
        "sEffectSummonCreature03",
        "sEffectSummonCreature04",
        "sEffectSummonCreature05",
    ];

    pub const WATERBREATHING: RefId = RefId::SizedString(StringRefId("WaterBreathing"));
    pub const SWIFTSWIM: RefId = RefId::SizedString(StringRefId("SwiftSwim"));
    pub const WATERWALKING: RefId = RefId::SizedString(StringRefId("WaterWalking"));
    pub const SHIELD: RefId = RefId::SizedString(StringRefId("Shield"));
    pub const FIRESHIELD: RefId = RefId::SizedString(StringRefId("FireShield"));
    pub const LIGHTNINGSHIELD: RefId = RefId::SizedString(StringRefId("LightningShield"));
    pub const FROSTSHIELD: RefId = RefId::SizedString(StringRefId("FrostShield"));
    pub const BURDEN: RefId = RefId::SizedString(StringRefId("Burden"));
    pub const FEATHER: RefId = RefId::SizedString(StringRefId("Feather"));
    pub const JUMP: RefId = RefId::SizedString(StringRefId("Jump"));
    pub const LEVITATE: RefId = RefId::SizedString(StringRefId("Levitate"));
    pub const SLOWFALL: RefId = RefId::SizedString(StringRefId("SlowFall"));
    pub const LOCK: RefId = RefId::SizedString(StringRefId("Lock"));
    pub const OPEN: RefId = RefId::SizedString(StringRefId("Open"));
    pub const FIREDAMAGE: RefId = RefId::SizedString(StringRefId("FireDamage"));
    pub const SHOCKDAMAGE: RefId = RefId::SizedString(StringRefId("ShockDamage"));
    pub const FROSTDAMAGE: RefId = RefId::SizedString(StringRefId("FrostDamage"));
    pub const DRAINATTRIBUTE: RefId = RefId::SizedString(StringRefId("DrainAttribute"));
    pub const DRAINHEALTH: RefId = RefId::SizedString(StringRefId("DrainHealth"));
    pub const DRAINMAGICKA: RefId = RefId::SizedString(StringRefId("DrainMagicka"));
    pub const DRAINFATIGUE: RefId = RefId::SizedString(StringRefId("DrainFatigue"));
    pub const DRAINSKILL: RefId = RefId::SizedString(StringRefId("DrainSkill"));
    pub const DAMAGEATTRIBUTE: RefId = RefId::SizedString(StringRefId("DamageAttribute"));
    pub const DAMAGEHEALTH: RefId = RefId::SizedString(StringRefId("DamageHealth"));
    pub const DAMAGEMAGICKA: RefId = RefId::SizedString(StringRefId("DamageMagicka"));
    pub const DAMAGEFATIGUE: RefId = RefId::SizedString(StringRefId("DamageFatigue"));
    pub const DAMAGESKILL: RefId = RefId::SizedString(StringRefId("DamageSkill"));
    pub const POISON: RefId = RefId::SizedString(StringRefId("Poison"));
    pub const WEAKNESSTOFIRE: RefId = RefId::SizedString(StringRefId("WeaknessToFire"));
    pub const WEAKNESSTOFROST: RefId = RefId::SizedString(StringRefId("WeaknessToFrost"));
    pub const WEAKNESSTOSHOCK: RefId = RefId::SizedString(StringRefId("WeaknessToShock"));
    pub const WEAKNESSTOMAGICKA: RefId = RefId::SizedString(StringRefId("WeaknessToMagicka"));
    pub const WEAKNESSTOCOMMONDISEASE: RefId =
        RefId::SizedString(StringRefId("WeaknessToCommonDisease"));
    pub const WEAKNESSTOBLIGHTDISEASE: RefId =
        RefId::SizedString(StringRefId("WeaknessToBlightDisease"));
    pub const WEAKNESSTOCORPRUSDISEASE: RefId =
        RefId::SizedString(StringRefId("WeaknessToCorprusDisease"));
    pub const WEAKNESSTOPOISON: RefId = RefId::SizedString(StringRefId("WeaknessToPoison"));
    pub const WEAKNESSTONORMALWEAPONS: RefId =
        RefId::SizedString(StringRefId("WeaknessToNormalWeapons"));
    pub const DISINTEGRATEWEAPON: RefId = RefId::SizedString(StringRefId("DisintegrateWeapon"));
    pub const DISINTEGRATEARMOR: RefId = RefId::SizedString(StringRefId("DisintegrateArmor"));
    pub const INVISIBILITY: RefId = RefId::SizedString(StringRefId("Invisibility"));
    pub const CHAMELEON: RefId = RefId::SizedString(StringRefId("Chameleon"));
    pub const LIGHT: RefId = RefId::SizedString(StringRefId("Light"));
    pub const SANCTUARY: RefId = RefId::SizedString(StringRefId("Sanctuary"));
    pub const NIGHTEYE: RefId = RefId::SizedString(StringRefId("NightEye"));
    pub const CHARM: RefId = RefId::SizedString(StringRefId("Charm"));
    pub const PARALYZE: RefId = RefId::SizedString(StringRefId("Paralyze"));
    pub const SILENCE: RefId = RefId::SizedString(StringRefId("Silence"));
    pub const BLIND: RefId = RefId::SizedString(StringRefId("Blind"));
    pub const SOUND: RefId = RefId::SizedString(StringRefId("Sound"));
    pub const CALMHUMANOID: RefId = RefId::SizedString(StringRefId("CalmHumanoid"));
    pub const CALMCREATURE: RefId = RefId::SizedString(StringRefId("CalmCreature"));
    pub const FRENZYHUMANOID: RefId = RefId::SizedString(StringRefId("FrenzyHumanoid"));
    pub const FRENZYCREATURE: RefId = RefId::SizedString(StringRefId("FrenzyCreature"));
    pub const DEMORALIZEHUMANOID: RefId = RefId::SizedString(StringRefId("DemoralizeHumanoid"));
    pub const DEMORALIZECREATURE: RefId = RefId::SizedString(StringRefId("DemoralizeCreature"));
    pub const RALLYHUMANOID: RefId = RefId::SizedString(StringRefId("RallyHumanoid"));
    pub const RALLYCREATURE: RefId = RefId::SizedString(StringRefId("RallyCreature"));
    pub const DISPEL: RefId = RefId::SizedString(StringRefId("Dispel"));
    pub const SOULTRAP: RefId = RefId::SizedString(StringRefId("Soultrap"));
    pub const TELEKINESIS: RefId = RefId::SizedString(StringRefId("Telekinesis"));
    pub const MARK: RefId = RefId::SizedString(StringRefId("Mark"));
    pub const RECALL: RefId = RefId::SizedString(StringRefId("Recall"));
    pub const DIVINEINTERVENTION: RefId = RefId::SizedString(StringRefId("DivineIntervention"));
    pub const ALMSIVIINTERVENTION: RefId = RefId::SizedString(StringRefId("AlmsiviIntervention"));
    pub const DETECTANIMAL: RefId = RefId::SizedString(StringRefId("DetectAnimal"));
    pub const DETECTENCHANTMENT: RefId = RefId::SizedString(StringRefId("DetectEnchantment"));
    pub const DETECTKEY: RefId = RefId::SizedString(StringRefId("DetectKey"));
    pub const SPELLABSORPTION: RefId = RefId::SizedString(StringRefId("SpellAbsorption"));
    pub const REFLECT: RefId = RefId::SizedString(StringRefId("Reflect"));
    pub const CURECOMMONDISEASE: RefId = RefId::SizedString(StringRefId("CureCommonDisease"));
    pub const CUREBLIGHTDISEASE: RefId = RefId::SizedString(StringRefId("CureBlightDisease"));
    pub const CURECORPRUSDISEASE: RefId = RefId::SizedString(StringRefId("CureCorprusDisease"));
    pub const CUREPOISON: RefId = RefId::SizedString(StringRefId("CurePoison"));
    pub const CUREPARALYZATION: RefId = RefId::SizedString(StringRefId("CureParalyzation"));
    pub const RESTOREATTRIBUTE: RefId = RefId::SizedString(StringRefId("RestoreAttribute"));
    pub const RESTOREHEALTH: RefId = RefId::SizedString(StringRefId("RestoreHealth"));
    pub const RESTOREMAGICKA: RefId = RefId::SizedString(StringRefId("RestoreMagicka"));
    pub const RESTOREFATIGUE: RefId = RefId::SizedString(StringRefId("RestoreFatigue"));
    pub const RESTORESKILL: RefId = RefId::SizedString(StringRefId("RestoreSkill"));
    pub const FORTIFYATTRIBUTE: RefId = RefId::SizedString(StringRefId("FortifyAttribute"));
    pub const FORTIFYHEALTH: RefId = RefId::SizedString(StringRefId("FortifyHealth"));
    pub const FORTIFYMAGICKA: RefId = RefId::SizedString(StringRefId("FortifyMagicka"));
    pub const FORTIFYFATIGUE: RefId = RefId::SizedString(StringRefId("FortifyFatigue"));
    pub const FORTIFYSKILL: RefId = RefId::SizedString(StringRefId("FortifySkill"));
    pub const FORTIFYMAXIMUMMAGICKA: RefId =
        RefId::SizedString(StringRefId("FortifyMaximumMagicka"));
    pub const ABSORBATTRIBUTE: RefId = RefId::SizedString(StringRefId("AbsorbAttribute"));
    pub const ABSORBHEALTH: RefId = RefId::SizedString(StringRefId("AbsorbHealth"));
    pub const ABSORBMAGICKA: RefId = RefId::SizedString(StringRefId("AbsorbMagicka"));
    pub const ABSORBFATIGUE: RefId = RefId::SizedString(StringRefId("AbsorbFatigue"));
    pub const ABSORBSKILL: RefId = RefId::SizedString(StringRefId("AbsorbSkill"));
    pub const RESISTFIRE: RefId = RefId::SizedString(StringRefId("ResistFire"));
    pub const RESISTFROST: RefId = RefId::SizedString(StringRefId("ResistFrost"));
    pub const RESISTSHOCK: RefId = RefId::SizedString(StringRefId("ResistShock"));
    pub const RESISTMAGICKA: RefId = RefId::SizedString(StringRefId("ResistMagicka"));
    pub const RESISTCOMMONDISEASE: RefId = RefId::SizedString(StringRefId("ResistCommonDisease"));
    pub const RESISTBLIGHTDISEASE: RefId = RefId::SizedString(StringRefId("ResistBlightDisease"));
    pub const RESISTCORPRUSDISEASE: RefId = RefId::SizedString(StringRefId("ResistCorprusDisease"));
    pub const RESISTPOISON: RefId = RefId::SizedString(StringRefId("ResistPoison"));
    pub const RESISTNORMALWEAPONS: RefId = RefId::SizedString(StringRefId("ResistNormalWeapons"));
    pub const RESISTPARALYSIS: RefId = RefId::SizedString(StringRefId("ResistParalysis"));
    pub const REMOVECURSE: RefId = RefId::SizedString(StringRefId("RemoveCurse"));
    pub const TURNUNDEAD: RefId = RefId::SizedString(StringRefId("TurnUndead"));
    pub const SUMMONSCAMP: RefId = RefId::SizedString(StringRefId("SummonScamp"));
    pub const SUMMONCLANNFEAR: RefId = RefId::SizedString(StringRefId("SummonClannfear"));
    pub const SUMMONDAEDROTH: RefId = RefId::SizedString(StringRefId("SummonDaedroth"));
    pub const SUMMONDREMORA: RefId = RefId::SizedString(StringRefId("SummonDremora"));
    pub const SUMMONANCESTRALGHOST: RefId = RefId::SizedString(StringRefId("SummonAncestralGhost"));
    pub const SUMMONSKELETALMINION: RefId = RefId::SizedString(StringRefId("SummonSkeletalMinion"));
    pub const SUMMONBONEWALKER: RefId = RefId::SizedString(StringRefId("SummonBonewalker"));
    pub const SUMMONGREATERBONEWALKER: RefId =
        RefId::SizedString(StringRefId("SummonGreaterBonewalker"));
    pub const SUMMONBONELORD: RefId = RefId::SizedString(StringRefId("SummonBonelord"));
    pub const SUMMONWINGEDTWILIGHT: RefId = RefId::SizedString(StringRefId("SummonWingedTwilight"));
    pub const SUMMONHUNGER: RefId = RefId::SizedString(StringRefId("SummonHunger"));
    pub const SUMMONGOLDENSAINT: RefId = RefId::SizedString(StringRefId("SummonGoldenSaint"));
    pub const SUMMONFLAMEATRONACH: RefId = RefId::SizedString(StringRefId("SummonFlameAtronach"));
    pub const SUMMONFROSTATRONACH: RefId = RefId::SizedString(StringRefId("SummonFrostAtronach"));
    pub const SUMMONSTORMATRONACH: RefId = RefId::SizedString(StringRefId("SummonStormAtronach"));
    pub const FORTIFYATTACK: RefId = RefId::SizedString(StringRefId("FortifyAttack"));
    pub const COMMANDCREATURE: RefId = RefId::SizedString(StringRefId("CommandCreature"));
    pub const COMMANDHUMANOID: RefId = RefId::SizedString(StringRefId("CommandHumanoid"));
    pub const BOUNDDAGGER: RefId = RefId::SizedString(StringRefId("BoundDagger"));
    pub const BOUNDLONGSWORD: RefId = RefId::SizedString(StringRefId("BoundLongsword"));
    pub const BOUNDMACE: RefId = RefId::SizedString(StringRefId("BoundMace"));
    pub const BOUNDBATTLEAXE: RefId = RefId::SizedString(StringRefId("BoundBattleAxe"));
    pub const BOUNDSPEAR: RefId = RefId::SizedString(StringRefId("BoundSpear"));
    pub const BOUNDLONGBOW: RefId = RefId::SizedString(StringRefId("BoundLongbow"));
    pub const EXTRASPELL: RefId = RefId::SizedString(StringRefId("ExtraSpell"));
    pub const BOUNDCUIRASS: RefId = RefId::SizedString(StringRefId("BoundCuirass"));
    pub const BOUNDHELM: RefId = RefId::SizedString(StringRefId("BoundHelm"));
    pub const BOUNDBOOTS: RefId = RefId::SizedString(StringRefId("BoundBoots"));
    pub const BOUNDSHIELD: RefId = RefId::SizedString(StringRefId("BoundShield"));
    pub const BOUNDGLOVES: RefId = RefId::SizedString(StringRefId("BoundGloves"));
    pub const CORPRUS: RefId = RefId::SizedString(StringRefId("Corprus"));
    pub const VAMPIRISM: RefId = RefId::SizedString(StringRefId("Vampirism"));
    pub const SUMMONCENTURIONSPHERE: RefId =
        RefId::SizedString(StringRefId("SummonCenturionSphere"));
    pub const SUNDAMAGE: RefId = RefId::SizedString(StringRefId("SunDamage"));
    pub const STUNTEDMAGICKA: RefId = RefId::SizedString(StringRefId("StuntedMagicka"));

    // Tribunal only
    pub const SUMMONFABRICANT: RefId = RefId::SizedString(StringRefId("SummonFabricant"));

    // Bloodmoon only
    pub const SUMMONWOLF: RefId = RefId::SizedString(StringRefId("SummonWolf"));
    pub const SUMMONBEAR: RefId = RefId::SizedString(StringRefId("SummonBear"));
    pub const SUMMONBONEWOLF: RefId = RefId::SizedString(StringRefId("SummonBonewolf"));
    pub const SUMMONCREATURE04: RefId = RefId::SizedString(StringRefId("SummonCreature04"));
    pub const SUMMONCREATURE05: RefId = RefId::SizedString(StringRefId("SummonCreature05"));
}
pub static MAGIC_EFFECT_ID_TO_INDEX: LazyLock<HashMap<RefId, i32>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for (i, id) in MagicEffect::MAGIC_EFFECT_IDS.iter().enumerate() {
        map.insert(id.clone(), i as i32);
    }
    map
});

pub enum MagnitudeDisplayType {
    None,
    Feet,
    Level,
    Percentage,
    Points,
    TimesInt,
}
