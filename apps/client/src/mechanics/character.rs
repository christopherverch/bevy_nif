use std::f32::consts::PI;

use crate::{
    camera::MWCamera,
    defs::{CellLookup, LocalPlayer, WorldSettings},
    mechanics::ActorIntent,
};
use crate::{
    defs::{
        Cell, CreatureStats, CreatureStatsFlags, CurrentCell, DrawState, GameSettings, Movement,
        Npc, Player, Stance,
    },
    esm::EsmStore,
    live_cell_ref::LiveCellRef,
    mechanics::const_defs::IS_MOVING_THRESHOLD,
    world::is_swimming,
};
use bevy::prelude::*;
use common::mechanics::{CharacterController, CharacterState, JumpingStateKind};
use esm_parser_lib::types::{flags::CreatureFlags, magic_effect::MagicEffect};
use mwphysics::{ActorPhysics, PositionInterpolation};
pub fn update_movement_component_from_intent(
    actor_q: Query<(&mut ActorIntent, &mut Movement, &mut CreatureStats), Changed<ActorIntent>>,
) {
    for (mut actor_intent, mut movement, mut creature_stats) in actor_q {
        movement.desired_movement.x = actor_intent.movement.x;
        movement.desired_movement.z = actor_intent.movement.y;
        if actor_intent.jump {
            movement.desired_movement.y = 1.0;
            // Reset this here because we poll on Update, so we can't know if it has been used in input
            // collection
            actor_intent.jump = false;
        }
        movement.speed_factor = actor_intent.movement.length();
        creature_stats.set_movement_flag(CreatureStatsFlags::RUN, actor_intent.run);
        creature_stats.set_movement_flag(CreatureStatsFlags::SNEAK, actor_intent.sneak);
    }
}
pub fn update_character_components(
    time: Res<Time<Fixed>>,
    character_controller_q: Query<(
        &mut CharacterController,
        &mut CreatureStats,
        &LiveCellRef,
        &CurrentCell,
        &mut Movement,
        &mut PositionInterpolation,
        &Transform,
        Option<&mut Player>,
        Option<&LocalPlayer>,
        Option<&ActorPhysics>,
        Option<&Npc>,
    )>,
    cell_entity_lookup: Res<CellLookup>,
    cells: Query<&Cell>,
    esm_store: Res<EsmStore>,
    cam_settings: Single<&MWCamera>,
    world_settings: Res<WorldSettings>,
    game_settings: Res<GameSettings>,
) {
    let delta_secs = time.delta_secs();
    for (
        mut character_controller,
        mut creature_stats,
        live_cell_ref,
        current_cell,
        mut character_movement,
        mut position_interpolation,
        transform,
        mut player_opt,
        local_player_opt,
        actor_physics_opt,
        npc_opt,
    ) in character_controller_q
    {
        let mut movement = Vec3::ZERO;
        update_magic_effects();
        let is_local_player = local_player_opt.is_some();
        let is_first_person_player = is_local_player && cam_settings.first_person;
        let godmode = is_local_player && world_settings.god_mode;
        let mut scale = transform.scale;
        if !game_settings.normalize_race_speed
            && let LiveCellRef::Npc(npc) = live_cell_ref
        {
            if let Some(race) = esm_store.races.get(&npc.race) {
                let weight = if npc.is_male() {
                    race.data.male_weight
                } else {
                    race.data.female_weight
                };
                scale *= weight;
            }
        }
        let is_actor = matches!(live_cell_ref, LiveCellRef::Npc(_));
        if is_actor && creature_stats.teleported {
            character_controller.movement.smoothed_speed = Vec2::ZERO;
            creature_stats.teleported = false;
        }
        if !is_actor {
            update_anim_queue();
        } else if !creature_stats.dead {
            let on_ground = actor_physics_opt
                .map(|actor_physics| {
                    actor_physics.on_ground && actor_physics.internal_collision_mode
                })
                .unwrap_or_default();
            let in_water = actor_physics_opt
                .and_then(|actor_physics| {
                    let cell_entity = cell_entity_lookup.0.get(&current_cell.coords)?;
                    let cell = cells.get(*cell_entity).ok()?;
                    Some(is_swimming(
                        transform.translation,
                        world_settings.swim_height_scale,
                        actor_physics.rendering_half_extents.y,
                        cell,
                    ))
                })
                .unwrap_or(false);

            let flying = is_flying(
                &creature_stats,
                &world_settings,
                live_cell_ref,
                is_local_player,
                is_actor,
                actor_physics_opt.is_some(),
            );
            let collision_enabled = actor_physics_opt
                .map(|actor_physics| actor_physics.internal_collision_mode)
                .unwrap_or(false);
            let sneak = creature_stats.is_stance_active(Stance::Sneak) && !flying && !in_water;
            let is_running = creature_stats.is_stance_active(Stance::Run) && !flying;
            let is_moving = character_movement.desired_movement.x.abs() > IS_MOVING_THRESHOLD
                || character_movement.desired_movement.z.abs() > IS_MOVING_THRESHOLD;
            if !in_water && !flying {
                // Force Jump
                if creature_stats
                    .movement_flags
                    .contains(CreatureStatsFlags::FORCE_JUMP)
                {
                    character_movement.desired_movement.y = if on_ground { 1.0 } else { 0.0 };
                }
                // Force Move Jump, only jump if they're otherwise moving
                if creature_stats
                    .movement_flags
                    .contains(CreatureStatsFlags::FORCE_MOVE_JUMP)
                    && is_moving
                {
                    character_movement.desired_movement.y = if on_ground { 1.0 } else { 0.0 };
                }
            }
            let rot = character_movement.desired_rotation;
            character_movement.desired_rotation = Vec3::ZERO;
            let mut move_vec = character_movement.desired_movement;
            dbg!(move_vec);
            character_movement.speed_factor = move_vec.length().min(1.0);
            move_vec = move_vec.normalize_or_zero();
            if game_settings.smooth_movement {
                handle_smooth_movement(
                    &mut move_vec,
                    transform,
                    &mut character_controller,
                    &mut character_movement,
                    is_first_person_player,
                    is_local_player,
                    delta_secs,
                    &game_settings,
                );
            }
            let mut effective_rotation = rot.y;
            let can_move = creature_stats.max_speed > 0.0;
            let is_biped = if npc_opt.is_some() {
                true
            } else {
                match live_cell_ref {
                    LiveCellRef::Creature(creature) => {
                        creature.flags.contains(CreatureFlags::BIPED)
                    }
                    _ => false,
                }
            };
            if !is_biped || !game_settings.turn_to_movement_direction || is_first_person_player {
                character_movement.is_strafing = move_vec.x.abs() > move_vec.z.abs() * 2.0;
                creature_stats.side_movement_angle = 0.0;
            } else if can_move {
                let mut target_movement_angle = if move_vec.z >= 0.0 {
                    -move_vec.x.atan2(move_vec.z)
                } else {
                    move_vec.x.atan2(move_vec.z)
                };
                character_movement.is_strafing = (creature_stats.draw_state != DrawState::Nothing
                    || in_water)
                    && target_movement_angle.abs() > 60.0_f32.to_radians();
                if character_movement.is_strafing {
                    target_movement_angle = 0.0;
                }
                let mut delta = target_movement_angle - creature_stats.side_movement_angle;
                let cos_delta = delta.cos();
                if (move_vec.z < 0.0) == character_controller.movement.is_moving_backward {
                    // slow down when turning
                    character_movement.speed_factor *= (cos_delta.max(0.0) + 0.3).min(1.0);
                }
                if delta.abs() < 20.0_f32.to_radians() {
                    character_controller.movement.is_moving_backward = move_vec.z < 0.0;
                }
                let max_delta = PI * delta_secs * (2.5 - cos_delta);
                delta = delta.clamp(-max_delta, max_delta);
                creature_stats.side_movement_angle += delta;
                effective_rotation += delta;
            }
            let speed = get_current_speed(is_actor, &character_movement, &creature_stats);
            move_vec.x *= speed;
            move_vec.z *= speed;
            if character_controller.is_knocked_out()
                || character_controller.is_knocked_down()
                || character_controller.is_recovery()
                || character_controller.is_scripted_anim_playing()
            {
                move_vec = Vec3::ZERO;
            }
            let mut move_state = CharacterState::None;
            let mut idle_state = CharacterState::None;
            let mut jump_state = JumpingStateKind::None;
            if move_vec.x != 0.0 || move_vec.z != 0.0 {
                // advance athletics
                if is_local_player {
                    if in_water {
                        character_controller.movement.seconds_of_swimming += delta_secs;
                        while character_controller.movement.seconds_of_swimming > 1.0 {
                            //TODO::
                            //cls.skillUsageSucceeded(mPtr, ESM::Skill::Athletics, ESM::Skill::Athletics_SwimOneSecond);
                            character_controller.movement.seconds_of_swimming -= 1.0;
                        }
                    } else if is_running && !sneak {
                        while character_controller.movement.seconds_of_running > 1.0 {
                            //TODO::
                            //cls.skillUsageSucceeded(mPtr, ESM::Skill::Athletics, ESM::Skill::Athletics_RunOneSecond);
                            character_controller.movement.seconds_of_running -= 1.0;
                        }
                    }
                }
                if !godmode {
                    // reduce fatigue
                    // TODO::
                }
            }
            let was_in_jump = character_controller.jumping.in_jump;
            character_controller.jumping.in_jump = false;
            let jump_height = get_jump_height();
            if jump_height <= 0.0 || sneak || in_water || flying || !collision_enabled {
                move_vec.y = 0.0;
                // Following code might assign some vertical movement regardless, need to reset this manually
                // This is used for jumping detection
                character_movement.desired_movement.y = 0.0;
            }
            if !in_water && !flying && collision_enabled {
                // In the air (either getting up —ascending part of jump— or falling).
                if !on_ground {
                    character_controller.jumping.in_jump = true;
                    jump_state = JumpingStateKind::InAir;
                    let jump_move_base = esm_store.gamesettings.find("fJumpMoveBase").as_float();
                    let jump_move_mult = esm_store.gamesettings.find("fJumpMoveMult").as_float();
                    //TODO::
                    let acrobatics = 50.0;
                    let mut factor = jump_move_base + jump_move_mult * acrobatics / 100.0;
                    factor = factor.min(1.0);
                    move_vec.x *= factor;
                    move_vec.z *= factor;
                    move_vec.y = 0.0;
                }
                // Started a jump
                else if character_controller.jumping.state != JumpingStateKind::InAir
                    && move_vec.y > 0.0
                {
                    character_controller.jumping.in_jump = true;
                    if move_vec.x == 0.0 && move_vec.z == 0.0 {
                        move_vec.y = jump_height;
                        dbg!(move_vec);
                    } else {
                        let mut lateral_movement = move_vec.xz();
                        lateral_movement = lateral_movement.normalize_or_zero();
                        move_vec = Vec3::new(lateral_movement.x, 1.0, lateral_movement.y)
                            * jump_height
                            * 0.707;
                        dbg!(move_vec);
                    }
                }
            }
            if !character_controller.jumping.in_jump {
                if character_controller.jumping.state == JumpingStateKind::InAir
                    && !flying
                    && collision_enabled
                    && was_in_jump
                {
                    jump_state = JumpingStateKind::Landing;
                    character_controller.jumping.handled_landing_animation = false;
                    let height = creature_stats.land(player_opt.as_deref_mut());
                    // TODO::
                    let mut health_lost = 0.0 / height;
                    if !in_water {
                        health_lost = get_fall_damage();
                    }
                    if health_lost > 0.0 {
                        //TODO::
                    }
                    if npc_opt.is_some() {
                        //TODO:: play land sound
                    }
                }
                move_vec.y = 0.0;
                move_state = get_character_state(
                    in_water,
                    is_running,
                    sneak,
                    character_movement.is_strafing,
                    is_biped,
                    is_first_person_player,
                    move_vec,
                    effective_rotation,
                );
                if game_settings.turn_to_movement_direction
                    && is_first_person_player
                    && is_biped
                    && (move_state == CharacterState::SwimRunForward
                        || move_state == CharacterState::SwimWalkForward
                        || move_state == CharacterState::SwimRunBack
                        || move_state == CharacterState::SwimWalkBack)
                {
                    // TODO::
                }
                //TODO:: swim correction
                if is_biped {
                    if character_controller.movement.turn_animation_threshold > 0.0 {
                        character_controller.movement.turn_animation_threshold -= delta_secs;
                        if move_state == CharacterState::TurnRight
                            || move_state == CharacterState::TurnLeft
                            || move_state == CharacterState::SwimTurnRight
                            || move_state == CharacterState::SwimTurnLeft
                        {
                            character_controller.movement.turn_animation_threshold = 0.05;
                        } else if move_state == CharacterState::None
                            && character_controller.is_turning()
                            && character_controller.movement.turn_animation_threshold > 0.0
                        {
                            move_state = character_controller.movement.state;
                        }
                    }
                }
                if move_state != CharacterState::None {
                    character_controller.clear_animation_queue();
                    jump_state = JumpingStateKind::None;
                }
                character_controller.update_anim_queue();
                if character_controller.animation.queue.is_empty() {
                    idle_state = CharacterState::SpecialIdle;
                } else if sneak && !character_controller.jumping.in_jump {
                    idle_state = CharacterState::IdleSneak;
                } else {
                    idle_state = CharacterState::Idle;
                }
                if in_water {
                    idle_state = CharacterState::IdleSwim;
                }
                if character_controller.is_turning() {
                    if delta_secs > 0.0 {
                        let turn_speed = (rot.y.abs() / delta_secs / PI).min(1.5);
                        character_controller.movement.applied_turn_anim_speed = turn_speed;
                    }
                } else if character_controller.movement.state != CharacterState::None
                    && character_controller.movement.adjust_anim_speed
                {
                    // Vanilla caps the played animation speed.
                    const MAX_SPEED_MULT: f32 = 10.0;
                    let speed_mult = speed / character_controller.movement.anim_speed;
                    character_controller.movement.applied_movement_anim_speed =
                        speed_mult.min(MAX_SPEED_MULT);
                    if character_controller.is_movement_animation_controlled(
                        game_settings.player_movement_ignores_animation,
                        player_opt.is_some(),
                    ) {
                        scale *= (speed_mult / MAX_SPEED_MULT).max(1.0);
                    }
                }
            }
            character_controller.movement.state = move_state;
            character_controller.vitals.idle_state = idle_state;
            character_controller.jumping.state = jump_state;
            movement = move_vec;
        } else if creature_stats.dead {
            // initial start of death animation for actors that started the game as dead
            // not done in constructor since we need to give scripts a chance to set the mSkipAnim flag
            if !character_controller.animation.skip_anim
                && character_controller.vitals.death_state != CharacterState::None
                && character_controller.vitals.current_death_group.is_empty()
            {
                // Fast-forward death animation to end for persisting corpses or corpses after end of death animation
                // TODO::
            }
            // TODO::
        }
        dbg!(movement);
        position_interpolation.velocity = movement;
        //        let movement_from_animation;
    }
}
// TODO::
fn update_magic_effects() {}
// TODO::
fn update_anim_queue() {}

fn is_flying(
    stats: &CreatureStats,
    world_settings: &WorldSettings,
    cell_ref: &LiveCellRef,
    is_local_player: bool,
    is_actor: bool,
    has_actor_physics: bool,
) -> bool {
    if !is_actor {
        return false;
    }
    if stats.dead {
        return false;
    }
    let is_paralyzed_not_godmode = !(is_local_player && world_settings.god_mode)
        && stats
            .magic_effects
            .get_or_default(&MagicEffect::PARALYZE)
            .modifier
            > 0.0;

    if is_paralyzed_not_godmode {
        return false;
    }
    match cell_ref {
        LiveCellRef::Creature(creature) => {
            if creature.flags.contains(CreatureFlags::FLIES) {
                return true;
            }
        }
        _ => {}
    }

    if stats
        .magic_effects
        .get_or_default(&MagicEffect::LEVITATE)
        .get_magnitude()
        > 0.0
        && world_settings.levitation_enabled
    {
        return true;
    }
    if !has_actor_physics {
        return true;
    }
    false
}
#[inline]
fn handle_smooth_movement(
    move_vec: &mut Vec3,
    transform: &Transform,
    character_controller: &mut CharacterController,
    character_movement: &mut Movement,
    is_first_person_player: bool,
    is_local_player: bool,
    delta_secs: f32,
    game_settings: &GameSettings,
) {
    let angle = transform.rotation.to_euler(EulerRot::XYZ).1;
    let target_speed =
        Vec2::from_angle(-angle).rotate(move_vec.xz()) * character_movement.speed_factor;
    let current_smoothed = character_controller.movement.smoothed_speed;
    let delta = target_speed - current_smoothed;
    let speed_delta = character_movement.speed_factor - current_smoothed.length();
    let delta_len = delta.length();
    let max_delta = if is_first_person_player {
        1.0
    } else if speed_delta.abs() < delta_len / 2.0 {
        // Turning is smooth for player, less for NPCs to hit path points
        delta_secs
            * (if is_local_player {
                1.0 / game_settings.smooth_movement_player_turning_delay
            } else {
                6.0
            })
    } else if is_local_player && speed_delta < -delta_len / 2.0 {
        // As soon as controls are released, mwinput switches player from running to walking.
        // So stopping should be instant for player, otherwise it causes a small twitch.
        1.0
    } else {
        // In all other cases speeding up and stopping are smooth.
        delta_secs * 3.0
    };
    let smoothed_delta = if delta_len > max_delta {
        delta * (max_delta / delta_len)
    } else {
        delta
    };
    character_controller.movement.smoothed_speed += smoothed_delta;

    // Rotate back to world space
    let new_speed_vec =
        Vec2::from_angle(angle).rotate(character_controller.movement.smoothed_speed);
    let original_length = new_speed_vec.length();
    character_movement.speed_factor = original_length;
    let direction = if original_length > 0.0 {
        new_speed_vec / original_length
    } else {
        new_speed_vec // Keep the zero-vector
    };
    move_vec.x = direction.x;
    move_vec.z = direction.y;
    const EPS: f32 = 0.001;
    let is_moving_backward = character_controller.movement.is_moving_backward;
    if character_movement.speed_factor < EPS {
        character_movement.speed_factor = 0.0;
        move_vec.x = 0.0;
        move_vec.z = 1.0;
    } else if (move_vec.z < 0.0) != is_moving_backward {
        if target_speed.length() < EPS
            || (character_movement.desired_movement.z < 0.0)
                == character_controller.movement.is_moving_backward
        {
            move_vec.z = if is_moving_backward { -EPS } else { EPS };
        }
    }
    *move_vec = move_vec.normalize_or_zero();
}
#[inline]
fn get_current_speed(
    is_actor: bool,
    character_movement: &Movement,
    creature_stats: &CreatureStats,
) -> f32 {
    if !is_actor {
        return 0.0;
    };
    let mut move_speed = creature_stats.max_speed * character_movement.speed_factor;
    if character_movement.is_strafing {
        move_speed *= 0.75;
    }
    move_speed
}
#[inline]
fn get_jump_height() -> f32 {
    //TODO::
    5.0
}
fn get_fall_damage() -> f32 {
    //TODO::
    1.0
}
fn get_character_state(
    in_water: bool,
    is_running: bool,
    sneak: bool,
    is_strafing: bool,
    is_biped: bool,
    is_first_person: bool,
    move_vec: Vec3,
    effective_rotation: f32,
) -> CharacterState {
    // 1. Strafe Logic
    if is_strafing && move_vec.x.abs() > 0.0 {
        return match (in_water, sneak, move_vec.x > 0.0) {
            (true, _, true) => {
                if is_running {
                    CharacterState::SwimRunRight
                } else {
                    CharacterState::SwimWalkRight
                }
            }
            (true, _, false) => {
                if is_running {
                    CharacterState::SwimRunLeft
                } else {
                    CharacterState::SwimWalkLeft
                }
            }
            (false, true, true) => CharacterState::SneakRight,
            (false, true, false) => CharacterState::SneakLeft,
            (false, false, true) => {
                if is_running {
                    CharacterState::RunRight
                } else {
                    CharacterState::WalkRight
                }
            }
            (false, false, false) => {
                if is_running {
                    CharacterState::RunLeft
                } else {
                    CharacterState::WalkLeft
                }
            }
        };
    }

    // 2. Forward/Backward Logic (if moving but not strafing)
    if move_vec.length_squared() > 0.0 {
        let forward = move_vec.z >= 0.0;
        return match (in_water, sneak, forward) {
            (true, _, true) => {
                if is_running {
                    CharacterState::SwimRunForward
                } else {
                    CharacterState::SwimWalkForward
                }
            }
            (true, _, false) => {
                if is_running {
                    CharacterState::SwimRunBack
                } else {
                    CharacterState::SwimWalkBack
                }
            }
            (false, true, true) => CharacterState::SneakForward,
            (false, true, false) => CharacterState::SneakBack,
            (false, false, true) => {
                if is_running {
                    CharacterState::RunForward
                } else {
                    CharacterState::WalkForward
                }
            }
            (false, false, false) => {
                if is_running {
                    CharacterState::RunBack
                } else {
                    CharacterState::WalkBack
                }
            }
        };
    }

    // 3. Idle / Turning Logic (if no movement detected)
    // It seems only bipedal actors use turning animations.
    // Also do not use turning animations in the first-person view and when sneaking.
    if !sneak && !is_first_person && is_biped {
        if effective_rotation > 0.0 {
            return if in_water {
                CharacterState::SwimTurnRight
            } else {
                CharacterState::TurnRight
            };
        } else if effective_rotation < 0.0 {
            return if in_water {
                CharacterState::SwimTurnLeft
            } else {
                CharacterState::TurnLeft
            };
        }
    }

    CharacterState::Idle
}
