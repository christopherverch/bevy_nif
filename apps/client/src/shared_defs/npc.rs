use bevy::prelude::*;
use common::consts::MW_UNITS_PER_METER;
use common::mechanics::CharacterController;
use esm_parser_lib::types::attribute::Attribute;
use esm_parser_lib::types::magic_effect::MagicEffect;
use esm_parser_lib::types::skill::SkillIndex;

use crate::defs::{DrawState, MWCell};
use crate::esm_ptr::EsmPtr;
use crate::live_cell_ref::LiveCellRef;
use crate::magic_effects::MagicEffects;
use crate::mechanics::npc_stats::NpcStats;
use crate::physics::actor::ActorPhysics;
use esm_parser_lib::types::gmst::GameSetting;
use std::sync::OnceLock;

use esm_parser_lib::RefId;
use esm_store::EsmStore;

use crate::shared_defs::actor::Actor;
use crate::shared_defs::common_funcs::{
    get_normalized_encumbrance, is_running_generic, is_sneaking, is_swimming,
};
use crate::shared_defs::container::Container;
use crate::{defs::WorldSettings, mechanics::creature_stats::CreatureStats};

#[allow(non_snake_case)]
pub struct NpcGmst {
    pub fMinWalkSpeed: EsmPtr<GameSetting>,
    pub fMaxWalkSpeed: EsmPtr<GameSetting>,
    pub fEncumberedMoveEffect: EsmPtr<GameSetting>,
    pub fSneakSpeedMultiplier: EsmPtr<GameSetting>,
    pub fAthleticsRunBonus: EsmPtr<GameSetting>,
    pub fBaseRunMultiplier: EsmPtr<GameSetting>,
    pub fMinFlySpeed: EsmPtr<GameSetting>,
    pub fMaxFlySpeed: EsmPtr<GameSetting>,
    pub fSwimRunBase: EsmPtr<GameSetting>,
    pub fSwimRunAthleticsMult: EsmPtr<GameSetting>,
    pub fJumpEncumbranceBase: EsmPtr<GameSetting>,
    pub fJumpEncumbranceMultiplier: EsmPtr<GameSetting>,
    pub fJumpAcrobaticsBase: EsmPtr<GameSetting>,
    pub fJumpAcroMultiplier: EsmPtr<GameSetting>,
    pub fJumpRunMultiplier: EsmPtr<GameSetting>,
    pub fWereWolfRunMult: EsmPtr<GameSetting>,
    pub fKnockDownMult: EsmPtr<GameSetting>,
    pub iKnockDownOddsMult: EsmPtr<GameSetting>,
    pub iKnockDownOddsBase: EsmPtr<GameSetting>,
    pub fCombatArmorMinMult: EsmPtr<GameSetting>,
}
pub static STATIC_GMST: OnceLock<NpcGmst> = OnceLock::new();
pub static STATIC_ENCUMBRANCE_STR_MULT: OnceLock<f32> = OnceLock::new();
/// populates the GMST struct's fields with the corresponding gmsts in the store
macro_rules! load_gmst {
    ($store:expr, $($field:ident),* $(,)?) => {
        NpcGmst {
            $(
                $field: EsmPtr($store.gamesettings.get(&RefId::from_str(stringify!($field))).unwrap()),
            )*
        }
    };
}
pub struct Npc;
impl Npc {
    pub fn get_max_speed(
        ref_position: Vec3,
        actor_physics_opt: Option<&ActorPhysics>,
        actor_character_controller: &CharacterController,
        cell_ref: &LiveCellRef,
        current_cell: &MWCell,
        esm_store: &EsmStore,
        npc_stats: &NpcStats,
        actor_container: &mut Container,
        creature_stats: &CreatureStats,
        world_settings: &WorldSettings,
        is_local_player: bool,
    ) -> f32 {
        if creature_stats.is_paralyzed(is_local_player, world_settings)
            || creature_stats.knocked_down()
            || creature_stats.is_dead()
        {
            return 0.0;
        }
        let gmst = Npc::get_gmst(esm_store);
        let capacity = Npc::get_capacity(esm_store, creature_stats);
        let encumbrance = Npc::get_encumbrance(
            npc_stats,
            creature_stats,
            actor_container,
            is_local_player,
            world_settings,
        );
        let normalized_encumbrance = get_normalized_encumbrance(capacity, encumbrance);
        let is_running = is_running_generic(
            ref_position,
            creature_stats,
            actor_physics_opt,
            Some(actor_character_controller),
            world_settings,
            cell_ref,
            current_cell,
            true,
            is_local_player,
        );

        let mut move_speed;
        if normalized_encumbrance > 1.0 {
            move_speed = 0.0;
        } else if creature_stats
            .magic_effects
            .get_or_default(MagicEffect::LEVITATE)
            .get_magnitude()
            > 0.0
            && world_settings.levitation_enabled
        {
            let mut fly_speed = 0.01
                * creature_stats
                    .attributes
                    .get(&Attribute::SPEED)
                    .unwrap()
                    .get_modified()
                + creature_stats
                    .magic_effects
                    .get_or_default(MagicEffect::LEVITATE)
                    .get_magnitude();
            fly_speed = gmst.fMinFlySpeed.as_float()
                + fly_speed * (gmst.fMaxFlySpeed.as_float() - gmst.fMinFlySpeed.as_float());
            fly_speed *= 1.0 - gmst.fEncumberedMoveEffect.as_float() * normalized_encumbrance;
            fly_speed = 0.0_f32.max(fly_speed);
            move_speed = fly_speed;
        } else if is_swimming(
            ref_position,
            world_settings,
            actor_physics_opt.map(|actor_physics| actor_physics.rendering_half_extents.y),
            current_cell,
        ) {
            move_speed = Npc::get_swim_speed(
                gmst,
                &creature_stats.magic_effects,
                ref_position,
                actor_physics_opt,
                cell_ref,
                current_cell,
                npc_stats,
                creature_stats,
                actor_character_controller,
                actor_container,
                esm_store,
                world_settings,
                is_local_player,
            )
        } else if is_running && !actor_character_controller.is_sneaking() {
            move_speed = Npc::get_run_speed(
                ref_position,
                actor_physics_opt,
                cell_ref,
                current_cell,
                npc_stats,
                creature_stats,
                actor_character_controller,
                actor_container,
                esm_store,
                world_settings,
                is_local_player,
            )
        } else {
            move_speed = Npc::get_walk_speed(
                ref_position,
                actor_physics_opt,
                cell_ref,
                current_cell,
                npc_stats,
                creature_stats,
                actor_character_controller,
                actor_container,
                esm_store,
                world_settings,
                is_local_player,
            )
        }
        if npc_stats.is_werewolf() && is_running && creature_stats.draw_state == DrawState::Nothing
        {
            move_speed *= gmst.fWereWolfRunMult.as_float();
        }
        return move_speed;
    }

    pub fn get_walk_speed(
        ref_position: Vec3,
        actor_physics_opt: Option<&ActorPhysics>,
        cell_ref: &LiveCellRef,
        current_cell: &MWCell,
        npc_stats: &NpcStats,
        creature_stats: &CreatureStats,
        character_controller: &CharacterController,
        actor_container: &mut Container,
        esm_store: &EsmStore,
        world_settings: &WorldSettings,
        is_local_player: bool,
    ) -> f32 {
        let gmst = Npc::get_gmst(esm_store);
        let normalized_encumbrance = Npc::get_normalized_encumbrance(
            esm_store,
            npc_stats,
            creature_stats,
            actor_container,
            is_local_player,
            world_settings,
        );
        let is_sneaking = is_sneaking(
            creature_stats,
            Some(character_controller),
            ref_position,
            actor_physics_opt,
            world_settings,
            cell_ref,
            current_cell,
            // Npcs are always actors
            true,
            is_local_player,
        );
        let mut walk_speed = gmst.fMinWalkSpeed.as_float()
            + 0.01
                * creature_stats
                    .attributes
                    .get(&Attribute::SPEED)
                    .unwrap()
                    .get_modified()
                * (gmst.fMaxWalkSpeed.as_float() - gmst.fMinWalkSpeed.as_float());
        walk_speed *= 1.0 - gmst.fEncumberedMoveEffect.as_float() * normalized_encumbrance;
        walk_speed = walk_speed.max(0.0);
        if is_sneaking {
            walk_speed *= gmst.fSneakSpeedMultiplier.as_float()
        }
        walk_speed / MW_UNITS_PER_METER
    }
    pub fn get_run_speed(
        ref_position: Vec3,
        actor_physics_opt: Option<&ActorPhysics>,
        cell_ref: &LiveCellRef,
        current_cell: &MWCell,
        npc_stats: &NpcStats,
        creature_stats: &CreatureStats,
        character_controller: &CharacterController,
        actor_container: &mut Container,
        esm_store: &EsmStore,
        world_settings: &WorldSettings,
        is_local_player: bool,
    ) -> f32 {
        let gmst = Npc::get_gmst(esm_store);
        let athletics_multiplier = 0.01 * get_skill(&SkillIndex::Athletics.to_refid(), npc_stats);

        Npc::get_walk_speed(
            ref_position,
            actor_physics_opt,
            cell_ref,
            current_cell,
            npc_stats,
            creature_stats,
            character_controller,
            actor_container,
            esm_store,
            world_settings,
            is_local_player,
        ) * (athletics_multiplier * gmst.fAthleticsRunBonus.as_float()
            + gmst.fBaseRunMultiplier.as_float())
    }
    pub fn get_swim_speed(
        gmst: &NpcGmst,
        magic_effects: &MagicEffects,
        ref_position: Vec3,
        actor_physics_opt: Option<&ActorPhysics>,
        cell_ref: &LiveCellRef,
        current_cell: &MWCell,
        npc_stats: &NpcStats,
        creature_stats: &CreatureStats,
        character_controller: &CharacterController,
        actor_container: &mut Container,
        esm_store: &EsmStore,
        world_settings: &WorldSettings,
        is_local_player: bool,
    ) -> f32 {
        let is_running = character_controller.is_running();
        let base_speed = if is_running {
            Npc::get_run_speed(
                ref_position,
                actor_physics_opt,
                cell_ref,
                current_cell,
                npc_stats,
                creature_stats,
                character_controller,
                actor_container,
                esm_store,
                world_settings,
                is_local_player,
            )
        } else {
            Npc::get_walk_speed(
                ref_position,
                actor_physics_opt,
                cell_ref,
                current_cell,
                npc_stats,
                creature_stats,
                character_controller,
                actor_container,
                esm_store,
                world_settings,
                is_local_player,
            )
        };
        return base_speed
            * (1.0
                + 0.01
                    * magic_effects
                        .get_or_default(MagicEffect::SWIFTSWIM)
                        .get_magnitude())
            * (gmst.fSwimRunBase.as_float()
                + 0.01
                    * get_skill(&SkillIndex::Athletics.to_refid(), npc_stats)
                    * gmst.fSwimRunAthleticsMult.as_float());
    }

    pub fn get_gmst(store: &EsmStore) -> &'static NpcGmst {
        STATIC_GMST.get_or_init(|| {
            load_gmst!(
                store,
                fMinWalkSpeed,
                fMaxWalkSpeed,
                fEncumberedMoveEffect,
                fSneakSpeedMultiplier,
                fAthleticsRunBonus,
                fBaseRunMultiplier,
                fMinFlySpeed,
                fMaxFlySpeed,
                fSwimRunBase,
                fSwimRunAthleticsMult,
                fJumpEncumbranceBase,
                fJumpEncumbranceMultiplier,
                fJumpAcrobaticsBase,
                fJumpAcroMultiplier,
                fJumpRunMultiplier,
                fWereWolfRunMult,
                fKnockDownMult,
                iKnockDownOddsMult,
                iKnockDownOddsBase,
                fCombatArmorMinMult
            )
        })
    }
    pub fn get_normalized_encumbrance(
        esm_store: &EsmStore,
        npc_stats: &NpcStats,
        creature_stats: &CreatureStats,
        actor_container: &mut Container,
        is_player: bool,
        world_settings: &WorldSettings,
    ) -> f32 {
        let capacity = Npc::get_capacity(esm_store, creature_stats);
        let encumbrance = Npc::get_encumbrance(
            npc_stats,
            creature_stats,
            actor_container,
            is_player,
            world_settings,
        );
        get_normalized_encumbrance(capacity, encumbrance)
    }
    pub fn get_capacity(esm_store: &EsmStore, creature_stats: &CreatureStats) -> f32 {
        STATIC_ENCUMBRANCE_STR_MULT.get_or_init(|| {
            esm_store
                .gamesettings
                .find("fEncumbranceStrMult")
                .as_float()
        });
        creature_stats
            .attributes
            .get(&Attribute::STRENGTH)
            .unwrap()
            .get_modified()
            * STATIC_ENCUMBRANCE_STR_MULT.get().unwrap()
    }
    pub fn get_encumbrance(
        npc_stats: &NpcStats,
        creature_stats: &CreatureStats,
        actor_container: &mut Container,
        is_player: bool,
        world_settings: &WorldSettings,
    ) -> f32 {
        if npc_stats.is_werewolf() {
            0.0
        } else {
            Actor::get_encumbrance(creature_stats, actor_container, is_player, world_settings)
        }
    }
}

fn get_skill<'a>(skill: &RefId, npc_stats: &'a NpcStats) -> f32 {
    npc_stats
        .skills
        .get(skill)
        .expect("skill not found")
        .attribute
        .get_modified()
}
