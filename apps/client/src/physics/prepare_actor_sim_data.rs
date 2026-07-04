use avian3d::prelude::CollisionLayers;
use bevy::prelude::*;
use esm_parser_lib::{RefId, types::magic_effect::MagicEffect};
use mwphysics::{ActorFrameData, ActorPhysics, PositionInterpolation};

use crate::{
    defs::{ColliderEntityLink, Movement, Player, WorldSettings},
    mechanics::creature_stats::CreatureStats,
};
use esm_store::EsmStore;
pub fn prepare_actor_simulation(
    actor_q: Query<(
        Entity,
        &mut Movement,
        &mut ActorPhysics,
        &mut CreatureStats,
        &ColliderEntityLink,
        //TODO:: &CurrentCell,
        Option<&mut ActorFrameData>,
        &mut PositionInterpolation,
        Option<&mut Player>,
    )>,
    collision_layers_q: Query<&CollisionLayers>,
    // TODO::    cell_query: Query<&CellStore>,
    world_settings: Res<WorldSettings>,
    esm_store: Res<EsmStore>,
    mut commands: Commands,
) {
    for (
        entity,
        mut move_controller,
        mut actor,
        mut stats,
        collider_entity_link,
        // TODO:: current_cell
        fd_opt,
        mut position_interpolation,
        mut player_opt,
    ) in actor_q
    {
        // 1. Get Water Data from the current Cell
        let water_level = -10000.0;
        let water_walking = false;
        /*
                if let Ok(cell) = cell_query.get(parent.get()) {
                    if cell.has_water {
                        water_level = cell.water_level;
                        // Water Walking check (Magnitude > 0)
                        if actor.collision_enabled {
                            water_walking = effects.get_magnitude(MagicEffect::WaterWalking) > 0.0;
                        }
                    }
                }
        */
        let collision_layers = collision_layers_q.get(collider_entity_link.0).unwrap();
        let old_filter_mask = &collision_layers.filters;
        let new_filter_mask = actor.set_can_water_walk(water_walking, old_filter_mask);
        if new_filter_mask != old_filter_mask.0 {
            commands.entity(entity).insert(CollisionLayers::new(
                collision_layers.memberships,
                new_filter_mask,
            ));
        }
        // 2. Calculate Slow Fall multiplier
        // Morrowind Math: 1.0 - (Magnitude * 0.005)
        // Magnitude 100 = 0.5 (half speed), Magnitude 200 = 0.0 (no falling)

        /*let slow_fall_mag = effects.get_magnitude(MagicEffect::SlowFall);
                fd.slow_fall_mult = (1.0 - (slow_fall_mag * 0.005)).clamp(0.0, 1.0);
        */
        let slow_fall_mult = 1.0
            - (stats
                .magic_effects
                .get_or_default(MagicEffect::SLOWFALL)
                .get_magnitude()
                * 0.005)
                .clamp(0.0, 1.0);

        // 3. Determine if "Inert" (Dead or Paralyzed)
        let is_player = player_opt.is_some();
        let god_mode = is_player && world_settings.god_mode;

        let paralyzed = stats
            .magic_effects
            .get_or_default(MagicEffect::PARALYZE)
            .modifier
            > 0.0;
        let inert = stats.dead || (!god_mode && paralyzed);
        // 4. Update the snapshot for the solver
        // TODO::
        let flying = false;
        // TODO::
        //mIsAquatic(actor.getPtr().getClass().isPureWaterCreature(actor.getPtr()))
        let is_aquatic = false;

        let swim_height_scale = esm_store
            .gamesettings
            .get(&RefId::from_str("fSwimHeightScale"))
            .expect("missing gamesetting fSwimHeightScale!")
            .as_float();
        let reset_fd = ActorFrameData::new(
            &actor,
            inert,
            water_walking,
            slow_fall_mult,
            water_level,
            is_player,
            swim_height_scale,
            flying,
            is_aquatic,
            position_interpolation.consume_velocity(),
        );

        // Completely reset the framedata to the newly created one
        if let Some(mut fd) = fd_opt {
            *fd = reset_fd;
        } else {
            commands.entity(entity).insert(reset_fd);
        }
        // 5. Handle Jump
        // If the actor has a jump requested, apply the upward impulse here
        handle_jump(
            &mut move_controller,
            player_opt.as_deref_mut(),
            &esm_store,
            &world_settings,
            &mut stats,
        );
    }
}
fn handle_jump(
    move_controller: &mut Movement,
    player_opt: Option<&mut Player>,
    esm_store: &EsmStore,
    world_settings: &WorldSettings,
    creature_stats: &mut CreatureStats,
) {
    if move_controller.desired_movement.y == 0.0 {
        return;
    }
    let is_player = player_opt.is_some();
    if let Some(player) = player_opt {
        //TODO:: ptr.getClass().skillUsageSucceeded(ptr, ESM::Skill::Acrobatics, ESM::Skill::Acrobatics_Jump);
        player.jumping = true;
    }
    if !is_player || !world_settings.god_mode {
        let fatigue_jump_base = esm_store.gamesettings.find("fFatigueJumpBase").as_float();
        let fatigue_jump_mult = esm_store.gamesettings.find("fFatigueJumpMult").as_float();
        //TODO::
        let normalized_encumbrance = 1.0;
        //           get_normalized_encumbrance(get_capacity(), get_encumbrance());

        let fatigue_decrease = fatigue_jump_base + normalized_encumbrance * fatigue_jump_mult;
        let fatigue = &mut creature_stats.stats.fatigue;
        fatigue.set_current(fatigue.current - fatigue_decrease, false, false);

        move_controller.desired_movement.y = 0.0;
    }
}
