use bevy::prelude::*;
use std::f32::consts::PI;

use crate::{
    animation::{
        AnimationContext, NifAnimatorLink,
        animation_helpers::get_anim_components_for_creature,
        name_resolver::{
            fallback_short_weapon_group, get_weapon_short_group, is_real_weapon,
            movement_state_to_anim_group, run_state_to_walk_state,
        },
        show_creature_weapons, show_npc_weapons,
    },
    camera::MWCamera,
    defs::{CellLookup, IdToEntityMap, LocalPlayer, WorldSettings},
    mechanics::{
        ActorIntent,
        creature_stats::{CreatureStats, CreatureStatsFlags},
        npc_stats::NpcStats,
    },
    physics::{actor::ActorPhysics, position_interpolate::PositionInterpolation},
    shared_defs::{common_funcs::is_swimming, container::Container, npc::Npc},
};
use crate::{
    defs::{CurrentCell, DrawState, GameSettings, MWCell, Movement, Player, Stance},
    live_cell_ref::LiveCellRef,
    mechanics::const_defs::IS_MOVING_THRESHOLD,
};
use bevy_nif::nif_animation::{
    BlendMask, NUM_DISCRETE_REGIONS, NifAnimator, REGION_INDEX_LEFT_ARM, REGION_INDEX_LOWER_BODY,
    bevy_types::{AnimType, Priority},
};
use common::consts::MW_UNITS_PER_METER;
use common::{
    events::{SoundEvent, SoundSource},
    mechanics::{CharacterController, CharacterState, JumpingStateKind, UpperBodyState},
};
use esm_parser_lib::types::{enums::WeaponType, flags::CreatureFlags, magic_effect::MagicEffect};
use esm_store::EsmStore;
use rand::RngExt;
pub fn update_movement_component_from_intent(
    actor_q: Query<
        (
            &mut ActorIntent,
            &mut Movement,
            &mut CreatureStats,
            &CharacterController,
            &PositionInterpolation,
            &LiveCellRef,
            &CurrentCell,
            &NpcStats,
            &mut Container,
            Option<&ActorPhysics>,
            Option<&LocalPlayer>,
        ),
        Changed<ActorIntent>,
    >,
    cells: Query<&MWCell>,
    cell_entity_lookup: Res<CellLookup>,
    esm_store: Res<EsmStore>,
    world_settings: Res<WorldSettings>,
) {
    for (
        mut actor_intent,
        mut movement,
        mut creature_stats,
        actor_character_controller,
        position_interpolation,
        live_cell_ref,
        currently_in_cell,
        npc_stats,
        mut actor_container,
        actor_physics_opt,
        local_player_opt,
    ) in actor_q
    {
        let current_cell = cells
            .get(*cell_entity_lookup.0.get(&currently_in_cell.coords).unwrap())
            .unwrap();
        movement.desired_movement.x = -actor_intent.movement.x;
        movement.desired_movement.z = actor_intent.movement.y;
        if actor_intent.jump {
            movement.desired_movement.y = 1.0;
            // We have to reset jump here rather than in input collection, because input collection runs on update
            // so the input might have changed before it gets to this fixedupdate function
            actor_intent.jump = false;
        }
        movement.speed_factor = actor_intent.movement.length();
        creature_stats.max_speed = Npc::get_max_speed(
            position_interpolation.current_position,
            actor_physics_opt,
            actor_character_controller,
            live_cell_ref,
            current_cell,
            &esm_store,
            npc_stats,
            &mut actor_container,
            &creature_stats,
            &world_settings,
            local_player_opt.is_some(),
        );
        // if is_strafing
        // creature_stats.
        creature_stats.set_movement_flag(CreatureStatsFlags::RUN, actor_intent.run);
        creature_stats.set_movement_flag(CreatureStatsFlags::SNEAK, actor_intent.sneak);
    }
}
pub fn update_character_components(
    time: Res<Time<Fixed>>,
    character_controller_q: Query<(
        Entity,
        &mut CharacterController,
        &mut CreatureStats,
        &NifAnimatorLink,
        &LiveCellRef,
        &CurrentCell,
        &mut Movement,
        &mut PositionInterpolation,
        &Transform,
        Option<&mut Player>,
        Option<&LocalPlayer>,
        Option<&ActorPhysics>,
    )>,
    cell_entity_lookup: Res<CellLookup>,
    cells: Query<&MWCell>,
    esm_store: Res<EsmStore>,
    anim_clips: Res<Assets<AnimationClip>>,
    mut animator_q: Query<(
        &mut AnimationPlayer,
        &mut NifAnimator,
        &AnimationGraphHandle,
    )>,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    id_to_entity_map: Res<IdToEntityMap>,

    cam_settings: Single<&MWCamera>,
    mut world_settings: ResMut<WorldSettings>,
    game_settings: Res<GameSettings>,
    mut commands: Commands,
) {
    let delta_secs = time.delta_secs();
    for (
        character_entity,
        mut character_controller,
        mut creature_stats,
        creature_animator_link,
        live_cell_ref,
        current_cell,
        mut character_movement,
        mut position_interpolation,
        transform,
        mut player_opt,
        local_player_opt,
        actor_physics_opt,
    ) in character_controller_q
    {
        let Some((_, mut animation_player, mut animator, graph_handle)) =
            get_anim_components_for_creature(
                creature_stats.entity_id,
                creature_animator_link.0,
                &id_to_entity_map,
                &mut animator_q,
            )
        else {
            continue;
        };
        let mut animation_context = AnimationContext {
            animation_player: &mut animation_player,
            nif_animator: &mut animator,
            anim_graphs: &mut anim_graphs,
            graph_handle,
        };

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
        let is_npc = is_actor;
        let is_player = player_opt.is_some();
        if is_actor && creature_stats.teleported {
            character_controller.movement.smoothed_speed = Vec2::ZERO;
            creature_stats.teleported = false;
        }
        if !is_actor {
            update_anim_queue();
        } else if !creature_stats.is_dead() {
            let on_ground = actor_physics_opt
                .map(|actor_physics| {
                    actor_physics.on_ground && actor_physics.internal_collision_mode
                })
                .unwrap_or_default();
            let Some(cell) = cell_entity_lookup
                .0
                .get(&current_cell.coords)
                .and_then(|cell_entity| cells.get(*cell_entity).ok())
            else {
                warn!("entity missing cell!");
                continue;
            };
            let in_water = actor_physics_opt
                .and_then(|actor_physics| {
                    Some(is_swimming(
                        transform.translation,
                        &world_settings,
                        Some(actor_physics.rendering_half_extents.y),
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
            let is_biped = {
                match live_cell_ref {
                    LiveCellRef::Npc(_npc) => true,
                    LiveCellRef::Creature(creature) => {
                        creature.flags.contains(CreatureFlags::BIPED)
                    }
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
            character_controller.movement.curr_move_speed = speed;
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
            let mut idle_state;
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
                    } else {
                        let mut lateral_movement = move_vec.xz();
                        lateral_movement = lateral_movement.normalize_or_zero();
                        move_vec = Vec3::new(lateral_movement.x, 1.0, lateral_movement.y)
                            * jump_height
                            * 0.707;
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
                    let height = creature_stats.land(player_opt.as_deref_mut());
                    // TODO::
                    let mut health_lost = 0.0 / height;
                    if !in_water {
                        health_lost = get_fall_damage();
                    }
                    if health_lost > 0.0 {
                        //TODO::
                    }
                    if is_npc {
                        let sound_source = if is_local_player {
                            SoundSource::LocalPlayer
                        } else {
                            SoundSource::Attached(character_entity)
                        };
                        commands.trigger(SoundEvent {
                            sound_source,
                            sound_path: "sounds/bodyfallMED.wav".to_string(),
                        });
                    }
                }
                if animation_context.is_playing(&character_controller.jumping.current_group) {
                    jump_state = JumpingStateKind::Landing;
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
            }
            if game_settings.turn_to_movement_direction
                && !is_first_person_player
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

            if !character_controller.animation.skip_anim {
                refresh_current_anims(
                    &mut creature_stats,
                    &mut character_controller,
                    &mut animation_context,
                    actor_physics_opt,
                    &mut world_settings,
                    &anim_clips,
                    live_cell_ref,
                    &mut commands,
                    cell,
                    idle_state,
                    jump_state,
                    move_state,
                    position_interpolation.current_position,
                    false, // TODO:: update_weapon_state()
                    is_actor,
                    is_npc,
                    is_player,
                    is_biped,
                );
            }
            // TODO:: the split of this part -> animation -> part 2 goes here

            if character_controller.is_turning() {
                if delta_secs > 0.0 {
                    let turn_speed = (rot.y.abs() / delta_secs / PI).min(1.5);
                    character_controller.movement.applied_turn_anim_speed = turn_speed;
                }
            } else if character_controller.movement.state != CharacterState::None
                && character_controller.movement.adjust_anim_speed
            {
                const MAX_SPEED_MULT: f32 = 10.0;
                let speed_mult = speed / character_controller.movement.applied_move_anim_speed;
                animation_context.adjust_anim_speed(
                    &character_controller.movement.current_group,
                    speed_mult.min(MAX_SPEED_MULT),
                );
                if character_controller.is_movement_animation_controlled(
                    game_settings.player_movement_ignores_animation,
                    is_player,
                ) {
                    scale *= (speed_mult / MAX_SPEED_MULT).max(1.0);
                }
            }

            movement = move_vec;
        } else if creature_stats.is_dead() {
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
    if stats.is_dead() {
        return false;
    }
    let is_paralyzed_not_godmode = !(is_local_player && world_settings.god_mode)
        && stats
            .magic_effects
            .get_or_default(MagicEffect::PARALYZE)
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
        .get_or_default(MagicEffect::LEVITATE)
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
    100.0
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

    CharacterState::None
}
fn refresh_current_anims(
    creature_stats: &mut CreatureStats,
    char_cont: &mut CharacterController,
    animation_context: &mut AnimationContext,
    actor_physics_opt: Option<&ActorPhysics>,
    world_settings: &mut WorldSettings,
    animations: &Res<Assets<AnimationClip>>,
    live_ref: &LiveCellRef,
    commands: &mut Commands,
    current_cell: &MWCell,
    new_idle: CharacterState,
    new_jump: JumpingStateKind,
    new_movement: CharacterState,
    current_pos: Vec3,
    force: bool,
    is_actor: bool,
    is_npc: bool,
    is_player: bool,
    is_biped: bool,
) {
    refresh_hit_recoil_anims(
        creature_stats,
        char_cont,
        animation_context,
        actor_physics_opt,
        world_settings,
        animations,
        commands,
        current_cell,
        current_pos,
        is_actor,
        is_npc,
        is_player,
    );
    refresh_jump_anims(
        char_cont,
        animation_context,
        animations,
        commands,
        new_jump,
        force,
        is_biped,
    );
    refresh_movement_anims(
        char_cont,
        animation_context,
        animations,
        commands,
        live_ref,
        new_movement,
        force,
        is_biped,
    );
    refresh_idle_anims(
        char_cont,
        animation_context,
        world_settings,
        animations,
        commands,
        new_idle,
        force,
        is_biped,
    );
}
pub fn refresh_hit_recoil_anims(
    creature_stats: &mut CreatureStats,
    char_cont: &mut CharacterController,
    animation_context: &mut AnimationContext,
    actor_physics_opt: Option<&ActorPhysics>,
    world_settings: &mut WorldSettings,
    animations: &Res<Assets<AnimationClip>>,
    commands: &mut Commands,
    current_cell: &MWCell,
    current_pos: Vec3,
    is_actor: bool,
    is_npc: bool,
    is_player: bool,
) {
    if !is_actor {
        return;
    };
    let knockout = creature_stats.stats.fatigue.current < 0.0
        || creature_stats.stats.fatigue.static_stat.base == 0.0;
    let recovery = creature_stats.hit_recovery;
    let knockdown = &mut creature_stats.knocked_down;
    let did_block = creature_stats.block && !knockout && !recovery && !*knockdown;
    let rendering_half_extents_y_opt =
        actor_physics_opt.map(|actor_physics| actor_physics.rendering_half_extents.y);
    let is_swimming = is_swimming(
        current_pos,
        world_settings,
        rendering_half_extents_y_opt,
        current_cell,
    );
    creature_stats.block = false;
    if is_player && char_cont.vitals.hit_state == CharacterState::Block && did_block {
        char_cont.vitals.hit_state = CharacterState::None;
        reset_current_idle_state(char_cont, animation_context);
    }
    if char_cont.vitals.hit_state != CharacterState::None {
        if !animation_context.is_playing(&char_cont.vitals.current_hit_group) {
            if char_cont.is_knocked_out()
                && char_cont.vitals.current_hit_group.is_empty()
                && knockout
            {
                return;
            }
            char_cont.vitals.hit_state = CharacterState::None;
            char_cont.vitals.current_hit_group.clear();
            *knockdown = false;
            creature_stats.hit_recovery = false;
            reset_current_idle_state(char_cont, animation_context);
        } else if char_cont.is_knocked_out() {
            animation_context.set_repeating(&char_cont.vitals.current_hit_group, knockout);
        }
    }
    if !knockout && !*knockdown && !recovery && !did_block {
        return;
    }
    let mut priorities = [Priority::Knockdown; NUM_DISCRETE_REGIONS];
    if knockout {
        char_cont.vitals.hit_state = if is_swimming {
            CharacterState::SwimKnockOut
        } else {
            CharacterState::KnockOut
        };
        creature_stats.knocked_down = true;
    } else if *knockdown {
        char_cont.vitals.hit_state = if is_swimming {
            CharacterState::SwimKnockOut
        } else {
            CharacterState::KnockOut
        };
    } else if recovery {
        char_cont.vitals.hit_state = if is_swimming {
            CharacterState::SwimHit
        } else {
            CharacterState::Hit
        };
    } else if did_block {
        char_cont.vitals.hit_state = CharacterState::Block;
        priorities = [Priority::Hit; NUM_DISCRETE_REGIONS];
        priorities[REGION_INDEX_LEFT_ARM] = Priority::Block;
        priorities[REGION_INDEX_LOWER_BODY] = Priority::Block;
    }
    char_cont.vitals.current_hit_group = hit_state_to_anim_group(char_cont.vitals.hit_state);
    if char_cont.is_recovery() {
        char_cont.vitals.current_hit_group = choose_random_group(
            &char_cont.vitals.current_hit_group,
            animation_context.nif_animator,
            &mut world_settings.rng,
        )
        .0;
        if char_cont.vitals.hit_state == CharacterState::SwimHit
            && !animation_context.has_animation(&char_cont.vitals.current_hit_group)
        {
            char_cont.vitals.current_hit_group = choose_random_group(
                &char_cont.vitals.current_hit_group,
                animation_context.nif_animator,
                &mut world_settings.rng,
            )
            .0;
        }
    }
    // Cancel upper body animations
    if char_cont.is_knocked_out() || char_cont.is_knocked_down() {
        if !char_cont.combat.current_weapon.is_empty() {
            animation_context.clear_animation(&char_cont.combat.current_weapon);
            dbg!("called clear from knocked_out");
        }
        if char_cont.combat.upper_body > UpperBodyState::WeaponEquipped {
            char_cont.combat.upper_body = UpperBodyState::WeaponEquipped;
            if char_cont.combat.weapon_type > WeaponType::None {
                if is_npc {
                    show_npc_weapons(true);
                } else {
                    // assume it's a creature
                    show_creature_weapons(true);
                }
            }
        } else if char_cont.combat.upper_body < UpperBodyState::WeaponEquipped {
            char_cont.combat.upper_body = UpperBodyState::None;
        }
    }
    if !animation_context.has_animation(&char_cont.vitals.current_hit_group) {
        char_cont.vitals.current_hit_group.clear();
        return;
    }

    let suffix = if did_block { ": block" } else { "" };
    animation_context.play_animation(
        animations,
        &char_cont.vitals.current_hit_group,
        suffix,
        priorities,
        BlendMask::all(),
        true,
        AnimType::Intro,
        1.0,
        0.0,
        0,
        commands,
    );
}
fn refresh_jump_anims(
    char_cont: &mut CharacterController,
    animation_context: &mut AnimationContext,
    animations: &Res<Assets<AnimationClip>>,
    commands: &mut Commands,
    new_jump: JumpingStateKind,
    force: bool,
    is_biped: bool,
) {
    if !force && new_jump == char_cont.jumping.state {
        return;
    }
    if new_jump == JumpingStateKind::None {
        if !char_cont.jumping.current_group.is_empty() {
            reset_current_idle_state(char_cont, animation_context);
        }
        reset_current_jump_state(char_cont, animation_context);
        return;
    }
    let weapon_short_group = get_weapon_short_group(char_cont.combat.weapon_type, is_biped);
    let mut jump_anim_name = format!("{}{}", "jump", weapon_short_group);
    let mut jump_mask = BlendMask::all();
    if !weapon_short_group.is_empty() && !animation_context.has_animation(&jump_anim_name) {
        jump_anim_name = fallback_short_weapon_group(
            char_cont.combat.weapon_type,
            "jump".to_string(),
            Some(&mut jump_mask),
            animation_context.nif_animator,
            is_biped,
        )
    }
    if !animation_context.has_animation(&jump_anim_name) {
        if !char_cont.jumping.current_group.is_empty() {
            reset_current_idle_state(char_cont, animation_context);
        }
        reset_current_jump_state(char_cont, animation_context);
        return;
    }
    let start_at_loop = if new_jump == char_cont.jumping.state {
        AnimType::Loop
    } else {
        AnimType::Intro
    };
    char_cont.jumping.state = new_jump;
    clear_state_animation(&mut char_cont.jumping.current_group, animation_context);
    char_cont.jumping.current_group = jump_anim_name;
    if char_cont.jumping.state == JumpingStateKind::InAir {
        animation_context.play_animation(
            animations,
            &char_cont.jumping.current_group,
            "",
            [Priority::Jump; 4],
            jump_mask,
            false,
            start_at_loop,
            1.0,
            0.0,
            u32::MAX,
            commands,
        );
    } else if char_cont.jumping.state == JumpingStateKind::Landing {
        animation_context.play_animation(
            animations,
            &char_cont.jumping.current_group,
            "",
            [Priority::Jump; 4],
            jump_mask,
            true,
            AnimType::Outro,
            1.0,
            0.0,
            u32::MAX,
            commands,
        );
    }
}
fn refresh_movement_anims(
    char_cont: &mut CharacterController,
    animation_context: &mut AnimationContext,
    animations: &Res<Assets<AnimationClip>>,
    commands: &mut Commands,
    live_ref: &LiveCellRef,
    new_movement: CharacterState,
    force: bool,
    is_biped: bool,
) {
    // Same state, if not forced just return
    if new_movement == char_cont.movement.state && !force {
        return;
    }

    let movement_anim_group = movement_state_to_anim_group(new_movement);
    if movement_anim_group.is_empty() {
        if !char_cont.movement.current_group.is_empty() {
            reset_current_idle_state(char_cont, animation_context);
        }
        reset_current_movement_state(char_cont, animation_context);
        return;
    }

    let mut movement_anim_name = movement_anim_group.to_string();
    char_cont.movement.state = new_movement;

    // Handle "swim" fallback
    let mut swim_pos = movement_anim_name.find("swim");
    if !animation_context.has_animation(&movement_anim_name) {
        if let Some(pos) = swim_pos {
            movement_anim_name.replace_range(pos..pos + 4, "");
            swim_pos = None;
        }
    }

    let mut move_mask = BlendMask::all();
    let weapon_short_group = if is_real_weapon(char_cont.combat.weapon_type) || is_biped {
        get_weapon_short_group(char_cont.combat.weapon_type, is_biped)
    } else {
        ""
    };

    // Apply weapon suffix to animation name
    if !swim_pos.is_some() && !weapon_short_group.is_empty() {
        let mut weap_movement_name =
            if char_cont.combat.weapon_type == WeaponType::Spell && char_cont.is_turning() {
                format!("{}{}", weapon_short_group, movement_anim_name)
            } else {
                format!("{}{}", movement_anim_name, weapon_short_group)
            };

        if !animation_context.has_animation(&weap_movement_name) {
            weap_movement_name = fallback_short_weapon_group(
                char_cont.combat.weapon_type,
                movement_anim_name,
                Some(&mut move_mask),
                animation_context.nif_animator,
                is_biped,
            )
        }
        movement_anim_name = weap_movement_name;
    }

    // Handle "run" to "walk" fallback
    if !animation_context.has_animation(&movement_anim_name) {
        if let Some(run_pos) = movement_anim_name.find("run") {
            movement_anim_name.replace_range(run_pos..run_pos + 3, "walk");
        }
        if !animation_context.has_animation(&movement_anim_name) {
            if !char_cont.movement.current_group.is_empty() {
                reset_current_idle_state(char_cont, animation_context);
            }
            reset_current_movement_state(char_cont, animation_context);
            return;
        }
    }

    // Get start point if repeating the same animation
    let mut start_point = 0.0;
    if !char_cont.movement.current_group.is_empty()
        && movement_anim_name == char_cont.movement.current_group
    {
        start_point = animation_context.get_anim_info(&char_cont.movement.current_group);
    }

    char_cont.movement.animation_has_movement = true;
    clear_state_animation(&mut char_cont.movement.current_group, animation_context);
    char_cont.movement.current_group = movement_anim_name.clone();

    // For non-flying creatures, MW uses the Walk animation to calculate the animation velocity
    // even if we are running. This must be replicated, otherwise the observed speed would differ drastically.
    char_cont.movement.adjust_anim_speed = true;
    if let LiveCellRef::Creature(creature) = live_ref {
        if !creature.flags.contains(CreatureFlags::FLIES) {
            let walk_state = run_state_to_walk_state(char_cont.movement.state);
            let anim = movement_state_to_anim_group(walk_state);
            char_cont.movement.applied_move_anim_speed =
                animation_context.get_velocity(anim) / MW_UNITS_PER_METER;
            if char_cont.movement.applied_move_anim_speed <= 1.0 / MW_UNITS_PER_METER {
                // Another bug: when using a fallback animation (e.g. RunForward as fallback to SwimRunForward),
                // then the equivalent Walk animation will not use a fallback, and if that animation doesn't exist
                // we will play without any scaling.
                // Makes the speed attribute of most water creatures totally useless.
                // And again, this can not be fixed without patching game data.
                char_cont.movement.adjust_anim_speed = false;
                char_cont.movement.applied_move_anim_speed = 1.0 / MW_UNITS_PER_METER;
            }
        }
    } else {
        char_cont.movement.applied_move_anim_speed =
            animation_context.get_velocity(&char_cont.movement.current_group) / MW_UNITS_PER_METER;
        if char_cont.movement.applied_move_anim_speed <= 1.0 / MW_UNITS_PER_METER {
            // The first person anims don't have any velocity to calculate a speed multiplier from.
            // We use the third person velocities instead.
            // FIXME: should be pulled from the actual animation, but it is not presently loaded.
            let sneaking = char_cont.movement.state == CharacterState::SneakForward
                || char_cont.movement.state == CharacterState::SneakBack
                || char_cont.movement.state == CharacterState::SneakLeft
                || char_cont.movement.state == CharacterState::SneakRight;
            char_cont.movement.applied_move_anim_speed = if sneaking {
                33.5452 / MW_UNITS_PER_METER
            } else if char_cont.is_running() {
                222.857 / MW_UNITS_PER_METER
            } else {
                154.064 / MW_UNITS_PER_METER
            };
            char_cont.movement.animation_has_movement = false;
        }
    }

    animation_context.play_animation(
        animations,
        &movement_anim_name,
        "",
        [Priority::Movement; 4],
        move_mask,
        false,
        AnimType::Intro,
        1.0,
        start_point,
        u32::MAX,
        commands,
    );
}

fn refresh_idle_anims(
    char_cont: &mut CharacterController,
    animation_context: &mut AnimationContext,
    world_settings: &mut WorldSettings,
    animations: &Res<Assets<AnimationClip>>,
    commands: &mut Commands,
    new_idle: CharacterState,
    force: bool,
    is_biped: bool,
) {
    if !force
        && new_idle == char_cont.vitals.idle_state
        && (animation_context.is_playing(&char_cont.vitals.current_idle_group)
            || !char_cont.animation.queue.is_empty())
    {
        return;
    }
    char_cont.vitals.idle_state = new_idle;
    let mut idle_group = idle_state_to_anim_group(new_idle);
    if idle_group.is_empty() {
        reset_current_idle_state(char_cont, animation_context);
        return;
    }
    let mut priority = get_idle_priority(char_cont.vitals.idle_state);
    let mut num_loops = u32::MAX;
    // Only play "idleswim" or "idlesneak" if they exist. Otherwise, fallback to
    // "idle"+weapon or "idle".
    let fallback = char_cont.vitals.idle_state != CharacterState::Idle
        && animation_context.has_animation(&idle_group);
    if fallback {
        priority = get_idle_priority(CharacterState::Idle);
        idle_group = idle_state_to_anim_group(CharacterState::Idle);
    }
    // Handle Weapon-specific Idle groups (e.g., "idle" + "1h")
    let is_base_idle = fallback
        || char_cont.vitals.idle_state == CharacterState::Idle
        || char_cont.vitals.idle_state == CharacterState::SpecialIdle;

    if is_base_idle {
        let weap_short_group = get_weapon_short_group(char_cont.combat.weapon_type, is_biped);
        if !weap_short_group.is_empty() {
            let mut weap_idle_group = format!("{}{}", idle_group, weap_short_group);

            if !animation_context.has_animation(&weap_idle_group) {
                weap_idle_group = fallback_short_weapon_group(
                    char_cont.combat.weapon_type,
                    idle_group,
                    None,
                    animation_context.nif_animator,
                    is_biped,
                );
            }
            idle_group = weap_idle_group;

            // play until the Loop Stop key 2 to 5 times, then play until the Stop key
            // this replicates original engine behavior for the "Idle1h" 1st-person animation
            num_loops = 1 + world_settings.rng.random_range(1..=4);
        }
    }

    // Validate animation exists
    if !animation_context.has_animation(&idle_group) {
        reset_current_idle_state(char_cont, animation_context);
        return;
    }

    // There is no need to restart anim if the new and old anims are the same.
    // Just update the number of loops.
    let mut start_point = 0.0;
    if char_cont.vitals.current_idle_group == idle_group {
        start_point = animation_context.get_anim_info(&char_cont.vitals.current_idle_group);
    }

    // Final transition
    clear_state_animation(&mut char_cont.vitals.current_idle_group, animation_context);
    char_cont.vitals.current_idle_group = idle_group.clone();

    animation_context.play_animation(
        animations,
        &idle_group,
        "",
        priority,
        BlendMask::all(),
        false,
        AnimType::Intro,
        1.0, // playback speed
        start_point,
        num_loops,
        commands,
    );
}
fn reset_current_idle_state(
    character_controller: &mut CharacterController,
    animation_context: &mut AnimationContext,
) {
    clear_state_animation(
        &mut character_controller.vitals.current_idle_group,
        animation_context,
    );
    character_controller.vitals.idle_state = CharacterState::None;
}
fn reset_current_jump_state(
    character_controller: &mut CharacterController,
    animation_context: &mut AnimationContext,
) {
    clear_state_animation(
        &mut character_controller.jumping.current_group,
        animation_context,
    );
    character_controller.jumping.state = JumpingStateKind::None;
}
fn reset_current_movement_state(
    character_controller: &mut CharacterController,
    animation_context: &mut AnimationContext,
) {
    dbg!(&character_controller.movement.current_group);
    clear_state_animation(
        &mut character_controller.movement.current_group,
        animation_context,
    );
    character_controller.movement.state = CharacterState::None;
    character_controller.movement.animation_has_movement = false;
}
fn clear_state_animation(anim_group: &mut String, animation_context: &mut AnimationContext) {
    if anim_group.is_empty() {
        return;
    }

    animation_context.clear_animation(anim_group);
    dbg!("called clear from clear_state");
    anim_group.clear();
}
fn hit_state_to_anim_group(state: CharacterState) -> String {
    match state {
        CharacterState::SwimHit => "swimhit",
        CharacterState::SwimKnockDown => "swimknockdown",
        CharacterState::SwimKnockOut => "swimknockout",
        CharacterState::Hit => "hit",
        CharacterState::KnockDown => "knockdown",
        CharacterState::KnockOut => "knockout",
        CharacterState::Block => "shield",
        _ => "",
    }
    .to_string()
}
fn idle_state_to_anim_group(state: CharacterState) -> String {
    match state {
        CharacterState::IdleSwim => "idleswim".to_string(),
        CharacterState::IdleSneak => "idlesneak".to_string(),
        CharacterState::Idle | CharacterState::SpecialIdle => "idle".to_string(),
        _ => String::new(),
    }
}

/// Returns the animation, and the roll that got us the animation
fn choose_random_group(
    prefix: &str,
    animator: &NifAnimator,
    rng: &mut impl RngExt,
) -> (String, usize) {
    let mut num_anims = 0;

    // Count how many variations exist (prefix1, prefix2, etc.)
    while animator
        .animation_definitions
        .contains_key(&format!("{}{}", prefix, num_anims + 1))
    {
        num_anims += 1;
    }

    if num_anims == 0 {
        return (prefix.to_string(), 0);
    }

    // Roll 1..=num_anims
    let roll = rng.random_range(1..=num_anims);

    (format!("{}{}", prefix, roll), roll)
}
fn get_idle_priority(state: CharacterState) -> [Priority; 4] {
    let mut priorities = [Priority::default(); 4];
    match state {
        CharacterState::IdleSwim => priorities = [Priority::SwimIdle; 4],
        CharacterState::IdleSneak => {
            priorities[REGION_INDEX_LOWER_BODY] = Priority::SneakIdleLowerBody
        }
        _ => {}
    }
    priorities
}
