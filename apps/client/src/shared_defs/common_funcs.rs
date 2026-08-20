use crate::{
    defs::{MWCell, Stance, WorldSettings},
    live_cell_ref::LiveCellRef,
    mechanics::creature_stats::CreatureStats,
    shared_defs::actor::Actor,
};
use bevy::prelude::*;
use common::mechanics::CharacterController;
use esm_parser_lib::types::{flags::CreatureFlags, magic_effect::MagicEffect};
use mwphysics::ActorPhysics;

pub fn get_normalized_encumbrance(capacity: f32, encumbrance: f32) -> f32 {
    // Intentional deviation: Morrowind doesn't do this
    // but handling (0 / 0) as 1.0 encumbrance feels like a clear oversight
    if encumbrance == 0.0 {
        return 0.0;
    }
    // Another deviation: handle (non-zero encumbrance / zero capacity) as "overencumbered"
    // Morrowind uses 1, but this means that for zero capacity,
    // normalized encumbrance cannot be used to detect overencumbrance
    if capacity == 0.0 {
        return 1.0 + 1e-6;
    }
    return encumbrance / capacity;
}
pub fn is_running_generic(
    ref_position: Vec3,
    creature_stats: &CreatureStats,
    actor_physics_opt: Option<&ActorPhysics>,
    // If it is an actor, this should be Some
    actor_character_controller_opt: Option<&CharacterController>,
    world_settings: &WorldSettings,
    cell_ref: &LiveCellRef,
    current_cell: &MWCell,
    is_actor: bool,
    is_local_player: bool,
) -> bool {
    if !creature_stats.is_stance_active(Stance::Run) {
        return false;
    }

    if let Some(actor_character_controller) = actor_character_controller_opt {
        if actor_character_controller.is_running() {
            return true;
        }
    }
    not_grounded_swimming_or_flying(
        ref_position,
        creature_stats,
        actor_physics_opt,
        world_settings,
        cell_ref,
        current_cell,
        is_actor,
        is_local_player,
    )
}
pub fn is_swimming(
    ref_position: Vec3,
    world_settings: &WorldSettings,
    // If it's an actor, the half extents of the actor, otherwise None
    rendering_half_extents_y_opt: Option<f32>,
    current_cell: &MWCell,
) -> bool {
    let rendering_half_extents_y = rendering_half_extents_y_opt.unwrap_or(0.0);
    is_underwater(
        ref_position,
        world_settings.swim_height_scale,
        rendering_half_extents_y,
        current_cell,
    )
}

pub fn is_underwater(
    mut ref_position: Vec3,
    height_ratio: f32,
    rendering_half_extents_y: f32,
    current_cell: &MWCell,
) -> bool {
    ref_position.y += height_ratio * 2.0 * rendering_half_extents_y;
    current_cell.has_water && ref_position.y < current_cell.water_level
}
pub fn is_flying(
    world_settings: &WorldSettings,
    cell_ref: &LiveCellRef,
    is_local_player: bool,
    is_actor: bool,
    creature_stats: Option<&CreatureStats>,
    has_actor_physics: bool,
) -> bool {
    if !is_actor {
        return false;
    }
    // if it's an actor, it has creature stats
    let creature_stats = creature_stats.unwrap();
    if creature_stats.is_dead() {
        return false;
    }
    let is_paralyzed_not_godmode = !(is_local_player && world_settings.god_mode)
        && creature_stats
            .magic_effects
            .get_or_default(MagicEffect::PARALYZE)
            .modifier
            > 0.0;

    if is_paralyzed_not_godmode {
        return false;
    }
    if let LiveCellRef::Creature(creature) = cell_ref {
        if creature.flags.contains(CreatureFlags::FLIES) {
            return true;
        }
    }
    let has_levitate = creature_stats
        .magic_effects
        .get_or_default(MagicEffect::LEVITATE)
        .get_magnitude()
        > 0.0;

    if has_levitate && world_settings.levitation_enabled {
        return true;
    }

    !has_actor_physics
}
pub fn is_sneaking(
    creature_stats: &CreatureStats,
    character_controller_opt: Option<&CharacterController>,
    ref_position: Vec3,
    actor_physics_opt: Option<&ActorPhysics>,
    world_settings: &WorldSettings,
    cell_ref: &LiveCellRef,
    current_cell: &MWCell,
    is_actor: bool,
    is_local_player: bool,
) -> bool {
    if !creature_stats.is_stance_active(Stance::Sneak) {
        return false;
    }

    if let Some(character_controller) = character_controller_opt {
        if character_controller.is_sneaking() {
            return true;
        }
    }
    not_grounded_swimming_or_flying(
        ref_position,
        creature_stats,
        actor_physics_opt,
        world_settings,
        cell_ref,
        current_cell,
        is_actor,
        is_local_player,
    )
}
fn not_grounded_swimming_or_flying(
    ref_position: Vec3,
    creature_stats: &CreatureStats,
    actor_physics_opt: Option<&ActorPhysics>,
    world_settings: &WorldSettings,
    cell_ref: &LiveCellRef,
    current_cell: &MWCell,
    is_actor: bool,
    is_local_player: bool,
) -> bool {
    let half_extents_y = actor_physics_opt.map(|p| p.rendering_half_extents.y);
    let swimming = is_swimming(ref_position, world_settings, half_extents_y, current_cell);
    let flying = is_flying(
        world_settings,
        cell_ref,
        is_local_player,
        is_actor,
        Some(creature_stats),
        actor_physics_opt.is_some(),
    );
    match actor_physics_opt {
        Some(physics) => !physics.is_on_ground() && !swimming && !flying,
        None => !swimming && !flying,
    }
}
