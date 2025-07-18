use bevy::{
    animation::{AnimationClip, AnimationPlayer, graph::AnimationGraphHandle},
    asset::Assets,
    log,
    time::Time,
};
use bevy_ecs::system::{Query, Res};
use bevy_nif::nif_animation::{
    AnimationDefinition, BlendMask, NUM_DISCRETE_REGIONS, NifAnimator,
    bevy_types::{ActiveAnimation, Priority},
};

use crate::client::{
    actor_defs::{ActorState, CharacterState, JumpingStateKind, UpperBodyState},
    actor_funcs::get_current_speed,
    const_defs::MW_SCALE_REDUCTION,
    defs::{CreatureStats, IdToEntityMap, Player},
};

use super::animation_helpers::get_anim_components_for_creature;

/// Checks all state variables of the actor and decides how they should be animated for this frame
/// Also calls functions to handle looping, stopping, etc of animations
pub fn animate_actors_using_state(
    time: Res<Time>,
    anim_clips: Res<Assets<AnimationClip>>,
    mut actor_query: Query<(&CreatureStats, &mut ActorState, Option<&Player>)>,
    mut animator_q: Query<(
        &mut AnimationPlayer,
        &mut NifAnimator,
        &AnimationGraphHandle,
    )>,
    id_to_entity_map: Res<IdToEntityMap>,
) {
    for (creature_stats, mut actor_state, player_opt) in actor_query.iter_mut() {
        let Some((_, mut animation_player, mut animator, _)) =
            get_anim_components_for_creature(&creature_stats, &id_to_entity_map, &mut animator_q)
        else {
            continue;
        };
        // First, process any ongoing transitions from the previous frame.
        handle_animation_transitions(
            &mut animation_player,
            &mut animator,
            &anim_clips,
            &mut actor_state,
        );
        let weapon_logic_took_control =
            handle_actor_weapon_actions(&mut animation_player, &mut animator, &mut actor_state);
        if weapon_logic_took_control {
            continue; // Skip jump/movement logic for this frame
        }
        //  We only handle ground movement if the character is not in a jumping state that overrides it.
        let jump_logic_took_control =
            handle_actor_jumping(&mut animation_player, &mut animator, &mut actor_state);

        if !jump_logic_took_control {
            handle_actor_ground_movement(
                &time,
                &anim_clips,
                &mut animation_player,
                &mut animator,
                &mut actor_state,
                player_opt.is_some(),
            );
        }
    }
}

/// Checks for finished non-looping animations and processes their transitions or removals.
///
/// This function is responsible for two primary state changes:
/// 1.  **Intro-to-Loop:** When an introductory animation (like `walkforward_intro`) finishes,
///     it seamlessly transitions to its corresponding looping animation (`walkforward_loop`).
/// 2.  **Outro Removal:** When a one-shot animation (like `jumpland_outro`) finishes, it's
///     removed, allowing the character to return to an idle or movement state.
fn handle_animation_transitions(
    animation_player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    anim_clips: &Assets<AnimationClip>,
    actor_state: &mut ActorState,
) {
    let mut transitions_to_perform = Vec::new();
    let mut animations_to_remove = Vec::new();

    // Step 1: Identify finished animations and decide their next action (transition, remove, or freeze).
    // This two-pass approach (collecting first, then mutating) avoids borrow checker issues.
    for (group_name, active_anim) in animator.active_animations.iter() {
        // We only care about animations that are not set to loop indefinitely.
        if active_anim.loop_count == u32::MAX {
            continue;
        }

        if let Some(anim_state) = animation_player.animation(active_anim.node_index) {
            if let Some(clip) = anim_clips.get(&active_anim.clip_handle) {
                let remaining_time = clip.duration() / anim_state.speed() - anim_state.elapsed();

                if remaining_time <= 0.001 {
                    if let Some(next_clip_name) = &active_anim.next_clip_name {
                        // Case 1 (Transition): The animation has a successor (e.g., intro -> loop).
                        transitions_to_perform.push((group_name.clone(), next_clip_name.clone()));
                    } else if active_anim.loop_count == 0 {
                        // Case 2 (Remove): A one-shot outro (e.g., jump land) finished.
                        animations_to_remove.push(group_name.clone());
                    }
                    // Case 3 (Freeze): loop_count is > 0 (e.g. 1 for a jump intro) and has no successor.
                    // We do nothing, letting it freeze on its last frame.
                }
            }
        }
    }

    // Step 2: Execute the collected state changes.
    for name in animations_to_remove {
        if name == "weaponequip" {
            actor_state.combat.upper_body_state = UpperBodyState::WeaponEquipped;
            log::info!("State -> WeaponEquipped");
        } else if name == "weaponunequip" {
            actor_state.combat.upper_body_state = UpperBodyState::None;
            log::info!("State -> None (Weapon Unequipped)");
        }
        clear_animation(animation_player, animator, actor_state, &name);
    }

    for (current_anim_name, next_anim_name) in transitions_to_perform {
        if let Some(mut transitioning_anim) =
            clear_animation(animation_player, animator, actor_state, &current_anim_name)
        {
            if let Some(definition) = animator.animation_definitions.get(&next_anim_name) {
                let original_priority = transitioning_anim.priorities[0];

                // Update the state to represent the new looping animation.
                transitioning_anim.clip_handle = definition.clip_handle.clone();
                transitioning_anim.node_index = definition.node_index;
                transitioning_anim.next_clip_name = None; // This is the loop, no successor.
                transitioning_anim.loop_count = u32::MAX; // Mark as a looping animation.

                play_animation(
                    animation_player,
                    animator,
                    actor_state,
                    &next_anim_name,
                    transitioning_anim,
                );

                // Update ActorState to track the new active animation group.
                if original_priority == Priority::Jump {
                    actor_state.jumping.current_animation_group = next_anim_name;
                } else {
                    actor_state.movement.current_animation_group = next_anim_name;
                }
            }
        }
    }
}

/// Manages high-priority jumping and landing animations.
/// Returns `true` if a jump/land animation was played or is ongoing, indicating
/// that ground movement logic should be skipped for this frame.
fn handle_actor_jumping(
    animation_player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    actor_state: &mut ActorState,
) -> bool {
    // --- 1. Handle a new jump event (from input) ---
    if !actor_state.jumping.handled_jump_animation {
        actor_state.jumping.handled_jump_animation = true;

        if let Some(name) = resolve_jump_intro_name(actor_state.combat.weapon_type, animator) {
            // A jump must override any current movement animations.
            clear_animation(
                animation_player,
                animator,
                actor_state,
                &actor_state.movement.current_animation_group.clone(),
            );
            clear_animation(
                animation_player,
                animator,
                actor_state,
                &actor_state.jumping.current_animation_group.clone(),
            );
            play_jump_animation(animation_player, animator, actor_state, &name, false);
        }
        return true; // A jump was initiated, block ground movement this frame.
    }

    // --- 2. Handle a new landing event ---
    if !actor_state.jumping.handled_landing_animation {
        actor_state.jumping.handled_landing_animation = true;

        if let Some(name) = resolve_jump_landing_name(actor_state.combat.weapon_type, animator) {
            clear_animation(
                animation_player,
                animator,
                actor_state,
                &actor_state.movement.current_animation_group.clone(),
            );
            clear_animation(
                animation_player,
                animator,
                actor_state,
                &actor_state.jumping.current_animation_group.clone(),
            );
            play_jump_animation(animation_player, animator, actor_state, &name, true);
        }
    }

    // --- 3. Handle falling off a ledge ---
    let final_jump_state = actor_state.jumping.state;
    let jump_anim_is_active = actor_state
        .jumping
        .current_animation_group
        .starts_with("jump");

    // If we are in the air, but no jump/fall animation is currently active,
    // it means we've fallen off a ledge.
    if final_jump_state == JumpingStateKind::InAir && !jump_anim_is_active {
        // When falling from a ledge, a ground movement animation (e.g., 'walkforward')
        // might still be active. We must explicitly clear it before playing the
        // 'jump' (falling) animation to ensure a clean visual transition.
        clear_animation(
            animation_player,
            animator,
            actor_state,
            &actor_state.movement.current_animation_group.clone(),
        );

        if let Some(name) = resolve_jump_intro_name(actor_state.combat.weapon_type, animator) {
            play_jump_animation(animation_player, animator, actor_state, &name, false);
        }
    }

    // --- 4. Ongoing State Cleanup ---
    if actor_state.jumping.state == JumpingStateKind::Landing {
        let is_playing_landing_outro = actor_state
            .movement
            .current_animation_group
            .ends_with("_outro");
        if !is_playing_landing_outro {
            actor_state.jumping.state = JumpingStateKind::None;
        }
    }

    if final_jump_state == JumpingStateKind::None
        && actor_state
            .jumping
            .current_animation_group
            .starts_with("jump")
    {
        clear_animation(
            animation_player,
            animator,
            actor_state,
            &actor_state.jumping.current_animation_group.clone(),
        );
    }

    // Block ground movement if the character is physically in the air.
    matches!(final_jump_state, JumpingStateKind::InAir)
}

/// A simplified helper to play a jump-related animation.
fn play_jump_animation(
    player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    actor_state: &mut ActorState,
    anim_name: &str,
    is_landing: bool,
) {
    if let Some(definition) = animator.animation_definitions.get(anim_name) {
        let new_animation = ActiveAnimation {
            clip_handle: definition.clip_handle.clone(),
            node_index: definition.node_index,
            next_clip_name: None,
            // Landing plays once and is removed (loop 0); jump intro plays once and freezes (loop 1).
            loop_count: if is_landing { 0 } else { 1 },
            // Landing is a movement priority; Jump is a higher priority.
            priorities: [if is_landing {
                Priority::Movement
            } else {
                Priority::Jump
            }; NUM_DISCRETE_REGIONS],
            blend_mask: BlendMask::ALL.bits(),
            animation_speed: 1.0,
        };

        play_animation(player, animator, actor_state, anim_name, new_animation);

        if is_landing {
            actor_state.movement.current_animation_group = anim_name.to_string();
            actor_state.jumping.current_animation_group.clear();
        } else {
            actor_state.jumping.current_animation_group = anim_name.to_string();
            actor_state.movement.current_animation_group.clear();
        }
    }
}

/// Manages idle, walking, running, and turning animations.
fn handle_actor_ground_movement(
    time: &Time,
    anim_clips: &Assets<AnimationClip>,
    animation_player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    actor_state: &mut ActorState,
    is_player: bool,
) {
    let weapon_type = actor_state.combat.weapon_type;

    let effective_move_state = determine_effective_move_state(
        time,
        anim_clips,
        animation_player,
        animator,
        actor_state,
        is_player,
    );

    let target_anim_name = resolve_movement_anim_name(effective_move_state, weapon_type, animator)
        .or_else(|| resolve_idle_anim_name(effective_move_state, weapon_type, animator));

    let new_name = target_anim_name.as_deref().unwrap_or("").to_string();
    let current_name = &actor_state.movement.current_animation_group.clone();

    // Check if a state change is needed. The state is "stable" if the new
    // animation is the same as the current one, is an intro to the current one,
    // or if we should let a landing animation finish.
    let is_stable_state = {
        if current_name.starts_with("jump") && current_name.ends_with("_outro") {
            // A landing animation is playing. We should let it finish unless the
            // player immediately begins a new linear movement (walk/run).
            // A new state of Idle or Turning should not interrupt the landing.
            !matches!(
                effective_move_state,
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
                    | CharacterState::SneakRight // etc. for all linear movements
            )
        } else if *current_name == new_name {
            true // The animation is already the correct one.
        } else if let Some(target_def) = animator.animation_definitions.get(&new_name) {
            // The new animation is an intro to the currently playing loop.
            target_def.next_clip_name.as_deref() == Some(current_name)
        } else {
            false
        }
    };

    if is_stable_state && !new_name.is_empty() {
        actor_state.movement.last_state = effective_move_state;
        return; // No change needed, let the current animation continue.
    }

    // State is not stable, so switch animations.
    clear_animation(animation_player, animator, actor_state, current_name);

    if !new_name.is_empty() {
        if let Some(definition) = animator.animation_definitions.get(&new_name) {
            let speed_multiplier = calculate_anim_speed_multiplier(
                effective_move_state,
                actor_state.is_running,
                actor_state.is_sneaking,
                definition,
                animator,
                weapon_type,
                get_current_speed(actor_state),
            );

            let new_animation = ActiveAnimation {
                clip_handle: definition.clip_handle.clone(),
                node_index: definition.node_index,
                next_clip_name: definition.next_clip_name.clone(),
                loop_count: if definition.next_clip_name.is_some() {
                    1
                } else {
                    u32::MAX
                },
                priorities: [Priority::Movement; NUM_DISCRETE_REGIONS],
                blend_mask: BlendMask::ALL.bits(),
                animation_speed: speed_multiplier,
            };

            play_animation(
                animation_player,
                animator,
                actor_state,
                &new_name,
                new_animation,
            );
        }
    }

    actor_state.movement.current_animation_group = new_name;
    actor_state.movement.last_state = effective_move_state;
}

/// Determines the character's animation state, applying special logic to
/// smooth out turning animations by enforcing a minimum duration. This prevents
/// visual flickering if the player taps a turn key.
fn determine_effective_move_state(
    time: &Time,
    anim_clips: &Assets<AnimationClip>,
    animation_player: &AnimationPlayer,
    animator: &NifAnimator,
    actor_state: &mut ActorState,
    is_player: bool,
) -> CharacterState {
    let initial_move_state = actor_state.movement.state;
    let last_frame_state = actor_state.movement.last_state;

    let is_currently_turning = matches!(
        last_frame_state,
        CharacterState::TurnLeft
            | CharacterState::TurnRight
            | CharacterState::SwimTurnLeft
            | CharacterState::SwimTurnRight
    );
    let new_state_is_idle = matches!(
        initial_move_state,
        CharacterState::Idle | CharacterState::IdleSneak | CharacterState::IdleSwim
    );

    // If we were turning last frame and are now idle, check if we should
    // continue the turn animation for a short period to make it look smoother.
    if is_currently_turning && new_state_is_idle {
        // Player logic: continue turning until the animation reaches a certain progress threshold.
        if is_player {
            let current_anim_name = &actor_state.movement.current_animation_group;
            let (intro_anim_name, is_in_loop_now) =
                if let Some(base) = current_anim_name.strip_suffix("_loop") {
                    (base.to_string(), true)
                } else {
                    (current_anim_name.clone(), false)
                };

            if let Some(intro_def) = animator.animation_definitions.get(&intro_anim_name) {
                if let Some(loop_anim_name) = &intro_def.next_clip_name {
                    if let Some(loop_def) = animator.animation_definitions.get(loop_anim_name) {
                        if let (Some(intro_clip), Some(loop_clip)) = (
                            anim_clips.get(&intro_def.clip_handle),
                            anim_clips.get(&loop_def.clip_handle),
                        ) {
                            let total_turn_duration = intro_clip.duration() + loop_clip.duration();
                            let mut total_elapsed_time = 0.0;
                            if is_in_loop_now {
                                if let Some(loop_anim_state) =
                                    animation_player.animation(loop_def.node_index)
                                {
                                    total_elapsed_time =
                                        intro_clip.duration() + loop_anim_state.elapsed();
                                }
                            } else if let Some(intro_anim_state) =
                                animation_player.animation(intro_def.node_index)
                            {
                                total_elapsed_time = intro_anim_state.elapsed();
                            }

                            if total_turn_duration > 0.0
                                && (total_elapsed_time / total_turn_duration) < 0.4
                            {
                                return last_frame_state; // Override state to continue turning.
                            }
                        }
                    }
                }
            }
        // NPC logic: use a simple timer.
        } else {
            if actor_state.movement.turn_animation_timer > 0.0 {
                actor_state.movement.turn_animation_timer -= time.delta_secs();
            }
            if matches!(
                initial_move_state,
                CharacterState::TurnLeft | CharacterState::TurnRight
            ) {
                actor_state.movement.turn_animation_timer = 0.05; // Reset timer on new turn.
            } else if actor_state.movement.turn_animation_timer > 0.0 {
                return last_frame_state; // Override state to continue turning.
            }
        }
    }

    // In all other cases, the effective state is just the current input state.
    initial_move_state
}

// --- Core Animation Control Functions ---

/// Stops an animation and cleans up all related state from the controllers.
///
/// Ensures that when an animation group is stopped, its state is correctly removed from:
/// 1.  Bevy's `AnimationPlayer` (to stop the visual playback).
/// 2.  The custom `NifAnimator` (to free up animation regions for other priorities).
/// 3.  The main `ActorState` component (to prevent stale state bugs).
///
/// Returns the `ActiveAnimation` struct of the removed animation if it existed.
fn clear_animation(
    animation_player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    actor_state: &mut ActorState,
    group_name: &str,
) -> Option<ActiveAnimation> {
    if group_name.is_empty() {
        return None;
    }

    if let Some(removed_anim) = animator.active_animations.remove(group_name) {
        log::trace!("Stopping animation group: '{}'", group_name);

        if let Some(definition) = animator.animation_definitions.get(group_name) {
            animation_player.stop(definition.node_index);
        }

        let blend_mask = BlendMask::from_bits_truncate(removed_anim.blend_mask);
        for i in 0..NUM_DISCRETE_REGIONS {
            let region_mask = BlendMask::from_bits_truncate(1 << i);
            if blend_mask.contains(region_mask)
                && animator.active_regions[i] == removed_anim.priorities[i]
            {
                animator.active_regions[i] = Priority::Idle;
            }
        }

        // Prevent stale state by clearing tracking fields in ActorState.
        if actor_state.jumping.current_animation_group == group_name {
            actor_state.jumping.current_animation_group.clear();
        }
        if actor_state.movement.current_animation_group == group_name {
            actor_state.movement.current_animation_group.clear();
        }

        return Some(removed_anim);
    }
    None
}

/// Plays a new animation, stopping any lower-priority animations that conflict with it.
fn play_animation(
    animation_player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    actor_state: &mut ActorState,
    group_key: &str,
    new_animation: ActiveAnimation,
) {
    let new_blend_mask = BlendMask::from_bits_truncate(new_animation.blend_mask);
    let mut keys_to_remove = Vec::new();

    // Find and collect all active animations that have a lower priority and affect the same body parts.
    'outer: for (existing_key, existing_anim) in &animator.active_animations {
        let existing_blend_mask = BlendMask::from_bits_truncate(existing_anim.blend_mask);
        if !new_blend_mask.intersects(existing_blend_mask) {
            continue; // No overlap in body parts, no conflict.
        }
        for i in 0..NUM_DISCRETE_REGIONS {
            let region_mask = BlendMask::from_bits_truncate(1 << i);
            if new_blend_mask.contains(region_mask) && existing_blend_mask.contains(region_mask) {
                if new_animation.priorities[i] >= existing_anim.priorities[i] {
                    keys_to_remove.push(existing_key.clone());
                    continue 'outer; // Mark for removal and check next animation.
                }
            }
        }
    }

    for key in keys_to_remove {
        clear_animation(animation_player, animator, actor_state, &key);
    }

    // Update the animator state with the new winning priorities.
    for i in 0..NUM_DISCRETE_REGIONS {
        let region_mask = BlendMask::from_bits_truncate(1 << i);
        if new_blend_mask.contains(region_mask) {
            animator.active_regions[i] = new_animation.priorities[i];
        }
    }

    let node_index = new_animation.node_index;
    let speed_multiplier = new_animation.animation_speed;

    animator
        .active_animations
        .insert(group_key.to_string(), new_animation.clone());
    let anim_state = animation_player
        .play(node_index)
        .set_speed(speed_multiplier);

    // loop_count == u32::MAX indicates a looping animation (e.g., walk_loop).
    // loop_count == 1 indicates an intro that plays once and freezes (e.g., jump_intro).
    // loop_count == 0 indicates an outro that plays once and is removed (e.g., jump_land).
    if new_animation.loop_count == u32::MAX {
        anim_state.repeat();
    } else {
        anim_state.set_repeat(bevy::animation::RepeatAnimation::Never);
    }

    println!(
        "Playing anim '{}' with speed {:.2} (Looping: {})",
        group_key,
        speed_multiplier,
        new_animation.loop_count == u32::MAX
    );
}

// --- Animation Name Resolution Helpers ---

/// Generates a prioritized list of animation names based on a base name and weapon type.
/// e.g., ("walkforward", weapon_type) -> ["walkforwardaxe", "walkforward1h", "walkforward"]
fn generate_weapon_anim_candidates(base_group: &str, weapon_type: i32) -> Vec<String> {
    let (specific_suffix, fallback_suffix) = get_weapon_suffixes(weapon_type);
    let mut candidates = Vec::with_capacity(3);

    if let Some(suffix) = specific_suffix {
        candidates.push(format!("{}{}", base_group, suffix));
    }
    if let Some(suffix) = fallback_suffix {
        if specific_suffix != fallback_suffix {
            // Avoid duplicates
            candidates.push(format!("{}{}", base_group, suffix));
        }
    }
    candidates.push(base_group.to_string());
    candidates
}

/// Finds the first valid animation name from a list of candidates.
fn find_first_valid_animation(candidates: &[String], animator: &NifAnimator) -> Option<String> {
    candidates
        .iter()
        .find(|name| animator.animation_definitions.contains_key(*name))
        .cloned()
}

/// Resolves a logical idle state into a final, playable animation group name.
fn resolve_idle_anim_name(
    state: CharacterState,
    weapon_type: i32,
    animator: &NifAnimator,
) -> Option<String> {
    let base_group = match state {
        CharacterState::Idle => "idle",
        CharacterState::IdleSneak => "idlesneak",
        CharacterState::IdleSwim => "idleswim",
        _ => return None, // Not an idle state
    };
    let candidates = generate_weapon_anim_candidates(base_group, weapon_type);
    find_first_valid_animation(&candidates, animator)
}

/// Resolves the JUMP INTRO animation group name (e.g., "jumphh").
fn resolve_jump_intro_name(weapon_type: i32, animator: &NifAnimator) -> Option<String> {
    let candidates = generate_weapon_anim_candidates("jump", weapon_type);
    find_first_valid_animation(&candidates, animator)
}

/// Resolves the JUMP LANDING animation group name (e.g., "jumphh_outro").
fn resolve_jump_landing_name(weapon_type: i32, animator: &NifAnimator) -> Option<String> {
    let base_candidates = generate_weapon_anim_candidates("jump", weapon_type);
    for base in base_candidates {
        let outro_name = format!("{}_outro", base);
        if animator.animation_definitions.contains_key(&outro_name) {
            return Some(outro_name);
        }
    }
    None
}

/// Resolves a logical movement state into a final, playable animation group name,
/// including handling run-to-walk and swim-to-walk fallbacks.
fn resolve_movement_anim_name(
    state: CharacterState,
    weapon_type: i32,
    animator: &NifAnimator,
) -> Option<String> {
    let mut base_group = match state {
        CharacterState::WalkForward => "walkforward",
        CharacterState::WalkBack => "walkback",
        CharacterState::WalkLeft => "walkleft",
        CharacterState::WalkRight => "walkright",
        CharacterState::SwimWalkForward => "swimwalkforward",
        CharacterState::SwimWalkBack => "swimwalkback",
        CharacterState::SwimWalkLeft => "swimwalkleft",
        CharacterState::SwimWalkRight => "swimwalkright",
        CharacterState::RunForward => "runforward",
        CharacterState::RunBack => "runback",
        CharacterState::RunLeft => "runleft",
        CharacterState::RunRight => "runright",
        CharacterState::SwimRunForward => "swimrunforward",
        CharacterState::SwimRunBack => "swimrunback",
        CharacterState::SwimRunLeft => "swimrunleft",
        CharacterState::SwimRunRight => "swimrunright",
        CharacterState::SneakForward => "sneakforward",
        CharacterState::SneakBack => "sneakback",
        CharacterState::SneakLeft => "sneakleft",
        CharacterState::SneakRight => "sneakright",
        CharacterState::TurnLeft => "turnleft",
        CharacterState::TurnRight => "turnright",
        CharacterState::SwimTurnLeft => "swimturnleft",
        CharacterState::SwimTurnRight => "swimturnright",
        _ => return None,
    };

    // --- Handle swim-to-walk/run fallback ---
    if base_group.starts_with("swim")
        && !animator
            .animation_definitions
            .keys()
            .any(|k| k.starts_with(base_group))
    {
        base_group = base_group.strip_prefix("swim").unwrap_or(base_group);
    }

    // --- Find primary candidates ---
    let candidates = generate_weapon_anim_candidates(base_group, weapon_type);
    if let Some(found) = find_first_valid_animation(&candidates, animator) {
        return Some(found);
    }

    // --- Handle run-to-walk fallback ---
    if base_group.starts_with("run") {
        if let Some(walk_base) = base_group.strip_prefix("run") {
            let walk_group = format!("walk{}", walk_base);
            let walk_candidates = generate_weapon_anim_candidates(&walk_group, weapon_type);
            if let Some(found) = find_first_valid_animation(&walk_candidates, animator) {
                return Some(found);
            }
        }
    }

    None // No suitable animation found
}

/// Returns the specific and fallback animation suffixes for a given weapon type.
fn get_weapon_suffixes(weapon_type: i32) -> (Option<&'static str>, Option<&'static str>) {
    // Corresponds to ESM::Weapon::Type enum
    match weapon_type {
        0 => (Some("hh"), None), // Hand-to-hand
        1 => (Some("shortblade"), Some("1h")),
        2 => (Some("longblade"), Some("1h")),
        3 => (Some("axe"), Some("1h")),
        4 => (Some("blunt"), Some("1h")),
        5 => (Some("longblade"), Some("2h")), // Longblade Two-Hand
        6 => (Some("axe"), Some("2h")),       // Axe Two-Hand
        7 => (Some("blunt"), Some("2h")),     // Blunt Two-Hand
        8 => (Some("spear"), Some("2h")),
        9 => (Some("bow"), Some("bow")),
        10 => (Some("crossbow"), Some("crossbow")),
        11 => (Some("throwing"), Some("throwing")),
        12 => (Some("spell"), None),
        13 => (Some("crossbow"), Some("1h")), // Crossbows use 1h movement
        _ => (None, None),                    // No weapon or unknown
    }
}

/// Converts a running state to its walking equivalent.
fn run_state_to_walk_state(state: CharacterState) -> CharacterState {
    match state {
        CharacterState::RunForward => CharacterState::WalkForward,
        CharacterState::RunBack => CharacterState::WalkBack,
        CharacterState::RunLeft => CharacterState::WalkLeft,
        CharacterState::RunRight => CharacterState::WalkRight,
        CharacterState::SwimRunForward => CharacterState::SwimWalkForward,
        CharacterState::SwimRunBack => CharacterState::SwimWalkBack,
        CharacterState::SwimRunLeft => CharacterState::SwimWalkLeft,
        CharacterState::SwimRunRight => CharacterState::SwimWalkRight,
        _ => state, // Not a running state, return as-is.
    }
}

/// Calculates the correct animation speed multiplier based on OpenMW's rules.
fn calculate_anim_speed_multiplier(
    state: CharacterState,
    is_running: bool,
    is_sneaking: bool,
    definition: &AnimationDefinition,
    animator: &NifAnimator,
    weapon_type: i32,
    character_speed: f32,
) -> f32 {
    if !matches!(
        state,
        CharacterState::WalkForward | CharacterState::RunForward /* etc. */
    ) {
        return 1.0; // Non-linear movements are not scaled.
    }

    let mut anim_base_speed;

    // For running, OpenMW uses the 'walk' animation's speed for calculation.
    if is_running {
        let walk_state = run_state_to_walk_state(state);
        anim_base_speed = resolve_movement_anim_name(walk_state, weapon_type, animator)
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
            33.5452
        } else if is_running {
            222.857
        } else {
            154.064
        };
    }

    let final_anim_speed = anim_base_speed / MW_SCALE_REDUCTION;
    if final_anim_speed > 1e-4 {
        character_speed / final_anim_speed
    } else {
        1.0
    }
}

/// Handles high-priority weapon actions like equipping and unequipping.
/// Returns `true` if it played an animation, to prevent other handlers from running.
fn handle_actor_weapon_actions(
    animation_player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    actor_state: &mut ActorState,
) -> bool {
    let anim_name = match actor_state.combat.upper_body_state {
        UpperBodyState::Equipping => "weaponequip".to_string(),
        UpperBodyState::Unequipping => "weaponunequip".to_string(),
        _ => return false, // No weapon action in progress.
    };

    if actor_state.combat.current_animation_group == anim_name {
        return true; // Animation is already in progress.
    }

    // New action: clear other animations and play the weapon anim.
    clear_animation(
        animation_player,
        animator,
        actor_state,
        &actor_state.movement.current_animation_group.clone(),
    );

    if let Some(definition) = animator.animation_definitions.get(&anim_name) {
        let new_animation = ActiveAnimation {
            clip_handle: definition.clip_handle.clone(),
            node_index: definition.node_index,
            next_clip_name: None,
            loop_count: 0, // One-shot animation.
            priorities: [Priority::Weapon; NUM_DISCRETE_REGIONS],
            blend_mask: BlendMask::UPPER_BODY.bits(),
            animation_speed: 1.0,
        };

        play_animation(
            animation_player,
            animator,
            actor_state,
            &anim_name,
            new_animation,
        );
        actor_state.combat.current_animation_group = anim_name;
    }
    true
}
