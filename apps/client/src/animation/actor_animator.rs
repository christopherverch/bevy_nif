use bevy::prelude::*;
use bevy_nif::nif_animation::{
    AnimationDefinition, BlendMask, NUM_DISCRETE_REGIONS, NifAnimator,
    bevy_types::{ActiveAnimation, Priority},
};
use common::mechanics::{CharacterController, CharacterState, JumpingStateKind, UpperBodyState};
use esm_parser_lib::types::enums::WeaponType;
use mwphysics::MW_UNITS_PER_METER;

use super::{
    animation_helpers::get_anim_components_for_creature,
    name_resolver::{
        get_follow_suffix, get_weapon_long_group, resolve_idle_anim_name,
        resolve_movement_anim_name, run_state_to_walk_state,
    },
    playback::AnimationContext,
};
use crate::defs::{CreatureStats, IdToEntityMap};
use crate::{
    animation::name_resolver::{resolve_jump_intro_name, resolve_jump_landing_name},
    const_defs::FRAME_DURATION,
};

/// Checks all state variables of the actor and decides how they should be animated for this frame
/// Also calls functions to handle looping, stopping, etc of animations
pub fn animate_actors_using_state(
    anim_clips: Res<Assets<AnimationClip>>,
    mut actor_query: Query<(&CreatureStats, &mut CharacterController)>,
    mut animator_q: Query<(
        &mut AnimationPlayer,
        &mut NifAnimator,
        &AnimationGraphHandle,
    )>,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    id_to_entity_map: Res<IdToEntityMap>,
) {
    for (creature_stats, mut character_controller) in actor_query.iter_mut() {
        if character_controller.jumping.handled_jumping_animation == false {}
        let Some((_, mut animation_player, mut animator, graph_handle)) =
            get_anim_components_for_creature(&creature_stats, &id_to_entity_map, &mut animator_q)
        else {
            continue;
        };
        let mut animation_context = AnimationContext {
            animation_player: &mut animation_player,
            nif_animator: &mut animator,
            character_controller: &mut character_controller,
            anim_graphs: &mut anim_graphs,
            graph_handle,
        };

        handle_vitals_animations(&mut animation_context);

        // First, process any ongoing transitions from the previous frame.
        handle_animation_transitions(&mut animation_context, &anim_clips);
        // Handle any high-priority combat actions like equipping.
        // This function will set an upper-body animation if needed.
        handle_actor_weapon_actions(&mut animation_context);

        // Handle all locomotion (jumping or ground movement).
        // The resolver has already ensured that only one of these can be active.
        handle_actor_jumping(&mut animation_context);
        handle_actor_ground_movement(&mut animation_context);
    }
}

/// Checks for finished non-looping animations and processes their transitions or removals.
fn handle_animation_transitions(
    animation_context: &mut AnimationContext,
    anim_clips: &Assets<AnimationClip>,
) {
    let mut transitions_to_perform = Vec::new();
    let mut animations_to_remove = Vec::new();

    // Step 1: Identify finished animations.
    for (group_name, active_anim) in animation_context.nif_animator.active_animations.iter() {
        // Looping animations continue forever
        if active_anim.loop_count == u32::MAX {
            continue;
        }
        if let Some(anim_state) = animation_context
            .animation_player
            .animation(active_anim.node_index)
        {
            if NifAnimator::is_finished(active_anim.clip_handle.clone(), anim_clips, anim_state) {
                if let Some(next_clip_name) = &active_anim.next_clip_name {
                    transitions_to_perform.push((group_name.clone(), next_clip_name.clone()));
                } else if active_anim.loop_count == 0 {
                    animations_to_remove.push(group_name.clone());
                }
            }
        }
    }

    // Step 2: Execute state changes for finished animations.
    for name in animations_to_remove {
        let finished_state = animation_context.character_controller.combat.upper_body;
        // If a windup animation finishes, we must not clear it while the state is
        // AttackWindUp (player is holding) OR AttackRelease (we need its elapsed time).
        if name.ends_with("_windup")
            && matches!(
                finished_state,
                UpperBodyState::AttackWindUp | UpperBodyState::AttackRelease
            )
        {
            continue;
        }
        // If the windup animation finishes, but we're still in the windup state,
        // it means the player is holding the charge. We do nothing and let it
        // stay frozen on the last frame.
        if matches!(finished_state, UpperBodyState::AttackWindUp) {
            continue;
        }
        if matches!(finished_state, UpperBodyState::Equipping) {
            animation_context.character_controller.combat.upper_body =
                UpperBodyState::WeaponEquipped;
            let movement_group = &animation_context
                .character_controller
                .movement
                .current_group;
            if !movement_group.is_empty() {
                if let Some(active_move_anim) = animation_context
                    .nif_animator
                    .active_animations
                    .get_mut(movement_group)
                {
                    active_move_anim.blend_mask = BlendMask::ALL;
                }
            }
        } else if matches!(finished_state, UpperBodyState::Unequipping) {
            animation_context.character_controller.combat.upper_body = UpperBodyState::None;
            animation_context.character_controller.combat.weapon_type = WeaponType::HandToHand;
        // --- Handle attack phase transitions ---
        } else if matches!(finished_state, UpperBodyState::AttackRelease) {
            // The release animation has finished, so move to the follow-through.
            animation_context.character_controller.combat.upper_body = UpperBodyState::AttackEnd;
        } else if matches!(finished_state, UpperBodyState::AttackEnd) {
            // The follow-through is done, return to a neutral ready state.
            animation_context.character_controller.combat.upper_body =
                UpperBodyState::WeaponEquipped;
        }
        // NOTE: AttackWindUp is no longer handled here, as its transition is driven by input, not time.
        animation_context.clear_animation(&name);

        if matches!(finished_state, UpperBodyState::Unequipping) {
            animation_context.clear_animation(
                &animation_context
                    .character_controller
                    .movement
                    .current_group
                    .clone(),
            );
        }
    }
    // Step 3: Handle intro-to-loop transitions.
    for (current_anim_name, next_anim_name) in transitions_to_perform {
        if let Some(mut transitioning_anim) = animation_context.clear_animation(&current_anim_name)
        {
            if let Some(definition) = animation_context
                .nif_animator
                .animation_definitions
                .get(&next_anim_name)
            {
                let original_priority = transitioning_anim.priorities[0];
                transitioning_anim.clip_handle = definition.clip_handle.clone();
                transitioning_anim.node_index = definition.node_index;
                transitioning_anim.next_clip_name = None;
                transitioning_anim.loop_count = u32::MAX;
                animation_context.play_animation(&next_anim_name, transitioning_anim, None);
                if original_priority == Priority::Jump {
                    animation_context.character_controller.jumping.current_group = next_anim_name;
                } else {
                    animation_context
                        .character_controller
                        .movement
                        .current_group = next_anim_name;
                }
            }
        }
    }
}

/// Manages high-priority jumping and landing animations.
/// Returns `true` if a jump/land animation was played or is ongoing, indicating
/// that ground movement logic should be skipped for this frame.
fn handle_actor_jumping(animation_context: &mut AnimationContext) -> bool {
    // --- 1. Handle a new jump event (from input) ---
    if animation_context.character_controller.jumping.in_jump
        && animation_context
            .character_controller
            .jumping
            .handled_jumping_animation
            == false
    {
        animation_context
            .character_controller
            .jumping
            .handled_jumping_animation = true;
        if let Some(name) = resolve_jump_intro_name(
            animation_context.character_controller.combat.weapon_type,
            animation_context.nif_animator,
        ) {
            // A jump must override any current movement animations.
            animation_context.clear_animation(
                &animation_context
                    .character_controller
                    .movement
                    .current_group
                    .clone(),
            );
            animation_context.clear_animation(
                &animation_context
                    .character_controller
                    .jumping
                    .current_group
                    .clone(),
            );
            play_jump_animation(animation_context, &name, false);
        }
        return true; // A jump was initiated, block ground animation
    }

    // --- 2. Handle a new landing event ---
    if !animation_context.character_controller.jumping.in_jump
        && !animation_context
            .character_controller
            .jumping
            .handled_landing_animation
    {
        animation_context
            .character_controller
            .jumping
            .handled_landing_animation = true;

        if let Some(name) = resolve_jump_landing_name(
            animation_context.character_controller.combat.weapon_type,
            animation_context.nif_animator,
        ) {
            animation_context.clear_animation(
                &animation_context
                    .character_controller
                    .movement
                    .current_group
                    .clone(),
            );
            animation_context.clear_animation(
                &animation_context
                    .character_controller
                    .jumping
                    .current_group
                    .clone(),
            );
            play_jump_animation(animation_context, &name, true);
        }
    }

    // --- 3. Handle falling off a ledge ---
    let final_jump_state = animation_context.character_controller.jumping.state;
    let jump_anim_is_active = animation_context
        .character_controller
        .jumping
        .current_group
        .starts_with("jump");

    // If we are in the air, but no jump/fall animation is currently active,
    // it means we've fallen off a ledge.
    if final_jump_state == JumpingStateKind::InAir && !jump_anim_is_active {
        // When falling from a ledge, a ground movement animation (e.g., 'walkforward')
        // might still be active. We must explicitly clear it before playing the
        // 'jump' (falling) animation to ensure a clean visual transition.
        animation_context.clear_animation(
            &animation_context
                .character_controller
                .movement
                .current_group
                .clone(),
        );

        if let Some(name) = resolve_jump_intro_name(
            animation_context.character_controller.combat.weapon_type,
            animation_context.nif_animator,
        ) {
            play_jump_animation(animation_context, &name, false);
        }
    }

    // --- 4. Ongoing State Cleanup ---
    if animation_context.character_controller.jumping.state == JumpingStateKind::Landing {
        let is_playing_landing_outro = animation_context
            .character_controller
            .movement
            .current_group
            .ends_with("_outro");
        if !is_playing_landing_outro {
            animation_context.character_controller.jumping.state = JumpingStateKind::None;
        }
    }

    if final_jump_state == JumpingStateKind::None
        && animation_context
            .character_controller
            .jumping
            .current_group
            .starts_with("jump")
    {
        animation_context.clear_animation(
            &animation_context
                .character_controller
                .jumping
                .current_group
                .clone(),
        );
    }

    matches!(final_jump_state, JumpingStateKind::InAir)
}

/// A simplified helper to play a jump-related animation.
fn play_jump_animation(
    animation_context: &mut AnimationContext,
    anim_name: &str,
    is_landing: bool,
) {
    if let Some(definition) = animation_context
        .nif_animator
        .animation_definitions
        .get(anim_name)
    {
        let new_animation = ActiveAnimation {
            clip_handle: definition.clip_handle.clone(),
            node_index: definition.node_index,
            next_clip_name: None,
            loop_count: if is_landing { 0 } else { 1 },
            priorities: [Priority::Jump; NUM_DISCRETE_REGIONS], // Use Jump priority for both
            blend_mask: BlendMask::ALL,                         // Request the whole body
            speed_mult: 1.0,
        };

        animation_context.play_animation(anim_name, new_animation, None);

        if is_landing {
            animation_context
                .character_controller
                .movement
                .current_group = anim_name.to_string();
            animation_context
                .character_controller
                .jumping
                .current_group
                .clear();
        } else {
            animation_context.character_controller.jumping.current_group = anim_name.to_string();
            animation_context
                .character_controller
                .movement
                .current_group
                .clear();
        }
    }
}

/// Manages idle, walking, running, and turning animations.
fn handle_actor_ground_movement(animation_context: &mut AnimationContext) {
    let is_in_combat_action = matches!(
        animation_context.character_controller.combat.upper_body,
        UpperBodyState::Equipping
            | UpperBodyState::Unequipping
            | UpperBodyState::AttackWindUp
            | UpperBodyState::AttackRelease
            | UpperBodyState::AttackEnd
    );

    let is_trying_to_idle = matches!(
        animation_context.character_controller.movement.state,
        CharacterState::Idle | CharacterState::IdleSneak | CharacterState::IdleSwim
    );

    if is_in_combat_action && is_trying_to_idle {
        // A combat action is taking place while stationary. The combat animation
        // should control the entire body. Do not play a conflicting idle animation.
        // We also ensure any previous movement animation is cleared.
        animation_context.clear_animation(
            &animation_context
                .character_controller
                .movement
                .current_group
                .clone(),
        );
        return;
    }
    let effective_move_state = animation_context.character_controller.movement.state;

    let target_anim_name = resolve_movement_anim_name(
        effective_move_state,
        animation_context.character_controller,
        animation_context.nif_animator,
    )
    .or_else(|| {
        resolve_idle_anim_name(
            effective_move_state,
            animation_context.character_controller,
            animation_context.nif_animator,
        )
    });

    let new_name = target_anim_name.as_deref().unwrap_or("").to_string();
    let current_name = &animation_context
        .character_controller
        .movement
        .current_group
        .clone();

    if current_name.starts_with("jump") && current_name.ends_with("_outro") {
        let wants_to_interrupt_with_movement = matches!(
            animation_context.character_controller.movement.state,
            CharacterState::WalkForward
                | CharacterState::WalkBack
                | CharacterState::WalkLeft
                | CharacterState::WalkRight
                | CharacterState::RunForward
                | CharacterState::RunBack
                | CharacterState::RunLeft
                | CharacterState::RunRight
                | CharacterState::SneakForward
                | CharacterState::SneakBack
                | CharacterState::SneakLeft
                | CharacterState::SneakRight
        );

        if !wants_to_interrupt_with_movement {
            // The player is idle or turning. Let the landing animation finish.
            // We do nothing by returning here, which leaves "jump_outro" playing.
            return;
        }
    }
    // Check if a state change is needed. The state is "stable" if the new
    // animation is the same as the current one, is an intro to the current one,
    // or if we should let a landing animation finish.
    let is_stable_state = {
        if *current_name == new_name {
            true // The animation is already the correct one.
        } else if let Some(target_def) = animation_context
            .nif_animator
            .animation_definitions
            .get(&new_name)
        {
            // The new animation is an intro to the currently playing loop.
            target_def.next_clip_name.as_deref() == Some(current_name)
        } else {
            false
        }
    };
    if is_stable_state && !new_name.is_empty() {
        if animation_context.character_controller.movement.state == effective_move_state {
            return; // No change needed, let the current animation continue.
        }
    }

    // State is not stable, so switch animations.
    animation_context.clear_animation(current_name);

    if !new_name.is_empty() {
        if let Some(definition) = animation_context
            .nif_animator
            .animation_definitions
            .get(&new_name)
        {
            // Return a higher value for running
            let speed_multiplier = calculate_anim_speed_multiplier(
                effective_move_state,
                animation_context.character_controller.is_running(),
                animation_context.character_controller.is_sneaking(),
                definition,
                animation_context.nif_animator,
                animation_context.character_controller,
            );
            let blend_mask = if matches!(
                animation_context.character_controller.combat.upper_body,
                UpperBodyState::Equipping | UpperBodyState::Unequipping
            ) {
                BlendMask::LOWER_BODY // Only affect legs if using a weapon
            } else {
                BlendMask::ALL // Affect the whole body otherwise
            };
            // Idle animations have Idle priority, not Movement priority.
            let priority = if new_name.starts_with("idle") {
                Priority::Default
            } else {
                Priority::Movement
            };
            let new_animation = ActiveAnimation {
                clip_handle: definition.clip_handle.clone(),
                node_index: definition.node_index,
                next_clip_name: definition.next_clip_name.clone(),
                loop_count: if definition.next_clip_name.is_some() {
                    1
                } else {
                    u32::MAX
                },
                priorities: [priority; NUM_DISCRETE_REGIONS],
                blend_mask,
                speed_mult: speed_multiplier,
            };

            animation_context.play_animation(&new_name, new_animation, None);
        }
    }

    animation_context
        .character_controller
        .movement
        .current_group = new_name;
    animation_context.character_controller.movement.state = effective_move_state;
}

/// Calculates the correct animation speed multiplier based on OpenMW's rules.
fn calculate_anim_speed_multiplier(
    state: CharacterState,
    is_running: bool,
    is_sneaking: bool,
    definition: &AnimationDefinition,
    animator: &NifAnimator,
    character_controller: &CharacterController,
) -> f32 {
    if matches!(
        state,
        CharacterState::Hit
            | CharacterState::KnockDown
            | CharacterState::None
            | CharacterState::Idle /* etc. */
    ) {
        return 1.0; // Non-linear movements are not scaled.
    }

    let mut anim_base_speed;

    // For running, OpenMW uses the 'walk' animation's speed for calculation.
    if is_running {
        let walk_state = run_state_to_walk_state(state);
        anim_base_speed = resolve_movement_anim_name(walk_state, &character_controller, animator)
            .and_then(|name| animator.animation_definitions.get(&name))
            .map_or(definition.base_velocity.length(), |def| {
                def.base_velocity.length()
            });
    } else {
        anim_base_speed = definition.base_velocity.length();
    }

    // If the animation data speed is too low, use a hardcoded fallback.
    if anim_base_speed <= 1.0 {
        anim_base_speed = if is_sneaking {
            33.5452 / MW_UNITS_PER_METER
        } else if is_running {
            222.857 / MW_UNITS_PER_METER
        } else {
            154.064 / MW_UNITS_PER_METER
        };
    }

    let scaled_anim_base_speed = anim_base_speed / MW_UNITS_PER_METER;
    if scaled_anim_base_speed > 1e-4 {
        // Vanilla caps the played animation speed.
        const MAX_SPEED_MULT: f32 = 10.0;
        (character_controller.movement.curr_move_speed / scaled_anim_base_speed).min(MAX_SPEED_MULT)
    } else {
        1.0
    }
}

/// Handles high-priority weapon actions like equipping, unequipping, and attacking.
fn handle_actor_weapon_actions(animation_context: &mut AnimationContext) -> bool {
    // --- NEW: Check if the character is currently moving ---
    let is_moving = !animation_context
        .character_controller
        .movement
        .current_group
        .is_empty()
        && !animation_context
            .character_controller
            .movement
            .current_group
            .starts_with("idle");

    // --- NEW: Update blend mask of the currently active combat animation ---
    if let Some(active_combat_anim) = animation_context
        .nif_animator
        .active_animations
        .get_mut(&animation_context.character_controller.combat.current_group)
    {
        // An attack is an upper-body action *if* the character is moving.
        let is_attack = matches!(
            animation_context.character_controller.combat.upper_body,
            UpperBodyState::AttackWindUp
                | UpperBodyState::AttackRelease
                | UpperBodyState::AttackEnd
        );

        let new_mask = if is_moving && is_attack {
            BlendMask::UPPER_BODY
        } else {
            BlendMask::ALL
        };

        // If the mask needs to change, update it and recalculate for all animations.
        if active_combat_anim.blend_mask != new_mask {
            active_combat_anim.blend_mask = new_mask;
            animation_context.recalculate_and_apply_masks();
        }
    }
    let weapon_group =
        get_weapon_long_group(animation_context.character_controller.combat.weapon_type);
    if weapon_group.is_empty() {
        return false;
    }
    if animation_context.character_controller.combat.upper_body == UpperBodyState::AttackRelease {
        let windup_anim_name = format!(
            "{}: {}_windup",
            weapon_group, animation_context.character_controller.combat.attack_type
        );

        if let Some(windup_def) = animation_context
            .nif_animator
            .animation_definitions
            .get(&windup_anim_name)
        {
            let min_charge_time = windup_def.min_attack_time_relative;

            // Get the elapsed time of the windup animation. If it's not playing,
            // assume it's finished and has a very high elapsed time.
            let elapsed_time = animation_context
                .animation_player
                .animation(windup_def.node_index)
                .map_or(f32::MAX, |anim| anim.elapsed());

            // If the windup animation hasn't played for the minimum duration, do nothing yet.
            // This effectively pauses the state machine while the character finishes the minimum charge.
            if elapsed_time < min_charge_time {
                // Return true to signify that the combat system is still in control,
                // preventing movement animations from playing over the held pose.
                return true;
            }
        }
    }
    let (action_part, is_attack) = match animation_context.character_controller.combat.upper_body {
        UpperBodyState::Equipping => ("equip".to_string(), false),
        UpperBodyState::Unequipping => ("unequip".to_string(), false),
        UpperBodyState::AttackWindUp => (
            format!(
                "{}_windup",
                animation_context.character_controller.combat.attack_type
            ),
            true,
        ),
        UpperBodyState::AttackRelease => (
            format!(
                "{}_release",
                animation_context.character_controller.combat.attack_type
            ),
            true,
        ),
        UpperBodyState::AttackEnd => (
            format!(
                "{}{}",
                animation_context.character_controller.combat.attack_type,
                get_follow_suffix(0.0)
            ),
            true,
        ),
        _ => return false,
    };

    let anim_name = format!("{}: {}", weapon_group, action_part);

    if animation_context.character_controller.combat.current_group == anim_name {
        return true;
    }

    let (clip_handle, node_index, min_hit_time) = {
        let Some(definition) = animation_context
            .nif_animator
            .animation_definitions
            .get(&anim_name)
        else {
            warn!("Could not find weapon animation definition: {}", anim_name);
            // Fallback to prevent getting stuck in a state with no animation
            match animation_context.character_controller.combat.upper_body {
                UpperBodyState::Equipping => {
                    animation_context.character_controller.combat.upper_body =
                        UpperBodyState::WeaponEquipped
                }
                UpperBodyState::Unequipping => {
                    animation_context.character_controller.combat.upper_body = UpperBodyState::None
                }
                UpperBodyState::AttackRelease => {
                    animation_context.character_controller.combat.upper_body =
                        UpperBodyState::AttackEnd
                }
                UpperBodyState::AttackEnd => {
                    animation_context.character_controller.combat.upper_body =
                        UpperBodyState::WeaponEquipped
                }
                _ => {}
            }
            return false;
        };
        (
            definition.clip_handle.clone(),
            definition.node_index,
            definition.min_hit_time_relative,
        )
    };

    // Determine if we *were* moving and if the *new* attack is an upper-body only animation.
    let was_moving = !animation_context
        .character_controller
        .movement
        .current_group
        .is_empty()
        && !animation_context
            .character_controller
            .movement
            .current_group
            .starts_with("idle");
    let mut start_time = None;
    if animation_context.character_controller.combat.upper_body == UpperBodyState::AttackRelease {
        let strength = animation_context
            .character_controller
            .combat
            .attack_strength;

        // Interpolate the target start time of the swing release animation
        // For strength=0, this will be min_hit_time. For strength=1, this will be 0.0.
        let target_start_time = (1.0 - strength) * min_hit_time;
        // Subtract 1 frame because it seeks 1 frame too late in the release animation otherwise
        // maybe due to float imprecision?
        // Clamp at 0.0 to prevent seeking to a negative time.
        let adjusted_start_time = (target_start_time - FRAME_DURATION).max(0.0);

        start_time = Some(adjusted_start_time);
    }
    let blend_mask = if was_moving && is_attack {
        BlendMask::UPPER_BODY
    } else {
        BlendMask::ALL
    };

    // Now, we only clear the movement animation if the new combat animation
    // needs to control the entire body.
    if blend_mask == BlendMask::ALL {
        animation_context.clear_animation(
            &animation_context
                .character_controller
                .movement
                .current_group
                .clone(),
        );
    }
    // Always clear the previous combat animation.
    animation_context.clear_animation(
        &animation_context
            .character_controller
            .combat
            .current_group
            .clone(),
    );
    let mut priorities = [Priority::WeaponLowerBody; NUM_DISCRETE_REGIONS];
    priorities[1] = Priority::Weapon; // Torso
    priorities[2] = Priority::Weapon; // Left Arm
    priorities[3] = Priority::Weapon; // Right Arm
    let new_animation = ActiveAnimation {
        clip_handle,
        node_index,
        next_clip_name: None,
        loop_count: 0,
        speed_mult: 1.0,
        blend_mask,
        priorities,
    };

    animation_context.play_animation(&anim_name, new_animation, start_time);
    animation_context.character_controller.combat.current_group = anim_name;

    true
}

/// Plays the hit animation if a vitals state is active. This runs after
/// the controller has already suppressed other states.
fn handle_vitals_animations(animation_context: &mut AnimationContext) {
    // If a hit animation is already playing, check if it has finished.
    if !animation_context
        .character_controller
        .vitals
        .current_hit_group
        .is_empty()
    {
        if !animation_context
            .nif_animator
            .active_animations
            .contains_key(
                &animation_context
                    .character_controller
                    .vitals
                    .current_hit_group,
            )
        {
            // Animation finished, so clear the state and tracking variable.
            animation_context.character_controller.vitals.hit_state = CharacterState::None;
            animation_context
                .character_controller
                .vitals
                .current_hit_group
                .clear();
        }
        return; // Let the current hit animation finish playing.
    }

    // If no hit state is active, there's nothing to do.
    if animation_context.character_controller.vitals.hit_state == CharacterState::None {
        return;
    }

    // A new hit state is active, so play the corresponding animation.
    let anim_name = match animation_context.character_controller.vitals.hit_state {
        CharacterState::Hit => "hit".to_string(),
        CharacterState::KnockDown => "knockdown".to_string(),
        // TODO:: other states
        _ => return,
    };

    if let Some(definition) = animation_context
        .nif_animator
        .animation_definitions
        .get(&anim_name)
    {
        let new_animation = ActiveAnimation {
            clip_handle: definition.clip_handle.clone(),
            node_index: definition.node_index,
            next_clip_name: None,
            loop_count: 0, // Play once.
            priorities: [Priority::Hit; NUM_DISCRETE_REGIONS],
            blend_mask: BlendMask::ALL,
            speed_mult: 1.0,
        };
        animation_context.play_animation(&anim_name, new_animation, None);
        animation_context
            .character_controller
            .vitals
            .current_hit_group = anim_name;
    }
}
