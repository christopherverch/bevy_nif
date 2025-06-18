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
use cdda_bevy::client::{
    actor_defs::{ActorState, CharacterState, JumpingStateKind},
    animation::get_anim_components_for_creature,
    defs::{CreatureStats, IdToEntityMap, Player},
};

use crate::{actor_funcs::get_current_speed, const_defs::MW_SCALE_REDUCTION};
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

    // --- Step 1: Identify all finished animations and decide their fate ---
    for (group_name, active_anim) in animator.active_animations.iter() {
        // We only care about animations that are not set to loop indefinitely.
        if active_anim.loop_count == u32::MAX {
            continue;
        }

        if let Some(anim_state) = animation_player.animation(active_anim.node_index) {
            if let Some(clip) = anim_clips.get(&active_anim.clip_handle) {
                let remaining_time = clip.duration() / anim_state.speed() - anim_state.elapsed();

                // Check if the animation has effectively finished.
                if remaining_time <= 0.001 {
                    if let Some(next_clip_name) = &active_anim.next_clip_name {
                        // Case 1: The animation has a defined successor. It's a transition.
                        transitions_to_perform.push((group_name.clone(), next_clip_name.clone()));
                    } else if active_anim.loop_count == 0 {
                        // Case 2: A one-shot outro (e.g., jump land). Remove it.
                        animations_to_remove.push(group_name.clone());
                    }
                    // Implicit Case 3: loop_count is > 0 and no next_clip_name exists.
                    // This is a "freeze-on-end" animation (e.g., jump intro). We do nothing,
                    // allowing it to remain active and frozen on its last frame.
                }
            }
        }
    }

    // --- Step 2: Execute the collected changes ---

    // Remove any one-shot animations that have finished.
    for name in animations_to_remove {
        clear_animation(animation_player, animator, actor_state, &name);
    }

    // Perform any intro-to-loop transitions.
    for (current_anim_name, next_anim_name) in transitions_to_perform {
        // Clearing the old animation returns its state, which we can reuse for the new one.
        if let Some(mut transitioning_anim) =
            clear_animation(animation_player, animator, actor_state, &current_anim_name)
        {
            if let Some(definition) = animator.animation_definitions.get(&next_anim_name) {
                // Preserve the priority from the original animation (e.g., Jump vs. Movement).
                let original_priority = transitioning_anim.priorities[0];

                // Update the state to represent the new looping animation.
                transitioning_anim.clip_handle = definition.clip_handle.clone();
                transitioning_anim.node_index = definition.node_index;
                transitioning_anim.next_clip_name = None; // The new animation is a loop.
                transitioning_anim.loop_count = u32::MAX; // Mark as a looping animation.
                let speed_multiplier = transitioning_anim.animation_speed;
                // use the old animation's speed
                play_animation(
                    animation_player,
                    animator,
                    actor_state,
                    &next_anim_name,
                    transitioning_anim,
                    speed_multiplier,
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
/// Manages high-priority jumping and landing animations.
/// Returns `true` if a jump/land animation was played or is ongoing, indicating
/// that ground movement logic should be skipped for this frame.
fn handle_actor_jumping(
    player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    actor_state: &mut ActorState,
) -> bool {
    // --- 1. Handle a new jump event ---
    if !actor_state.jumping.handled_jump_animation {
        // The physics system has signaled a new jump. We must handle it now.
        actor_state.jumping.handled_jump_animation = true; // Consume the event flag immediately.

        if let Some(name) = resolve_jump_intro_name(actor_state.combat.weapon_type, animator) {
            // A jump animation must override any current movement animations.
            clear_animation(
                player,
                animator,
                actor_state,
                &actor_state.movement.current_animation_group.clone(),
            );
            clear_animation(
                player,
                animator,
                actor_state,
                &actor_state.jumping.current_animation_group.clone(),
            );

            play_jump_animation(player, animator, actor_state, &name, false);
        }
        return true; // Jump animation takes priority.
    }

    // --- 2. Handle a new landing event ---
    if !actor_state.jumping.handled_landing_animation {
        // The physics system has signaled a landing.
        actor_state.jumping.handled_landing_animation = true; // Consume the event flag.

        // Check if the character is trying to move, as in the previous logic.
        let is_moving_on_land = !matches!(
            actor_state.movement.state,
            CharacterState::Idle
                | CharacterState::IdleSneak
                | CharacterState::IdleSwim
                | CharacterState::TurnLeft
                | CharacterState::TurnRight
                | CharacterState::SwimTurnLeft
                | CharacterState::SwimTurnRight
        );

        if is_moving_on_land {
            // Player wants to move immediately. We have "handled" the landing by ignoring
            // the animation, so we let ground logic take over.
            return false;
        }

        // Play the landing animation.
        if let Some(name) = resolve_jump_landing_name(actor_state.combat.weapon_type, animator) {
            // Landing animation also overrides any lingering jump animations.
            clear_animation(
                player,
                animator,
                actor_state,
                &actor_state.movement.current_animation_group.clone(),
            );
            clear_animation(
                player,
                animator,
                actor_state,
                &actor_state.jumping.current_animation_group.clone(),
            );

            play_jump_animation(player, animator, actor_state, &name, true);
        }
        return true; // Landing animation takes priority over idle.
    }

    // --- 3. Ongoing State Cleanup ---
    // If a jump intro animation is stuck playing while we are on the ground, clear it.
    // This prevents the character from being frozen in a jump pose.
    let final_jump_state = actor_state.jumping.state;
    let jump_anim_group = &actor_state.jumping.current_animation_group;
    if final_jump_state == JumpingStateKind::None
        && jump_anim_group.starts_with("jump")
        && !jump_anim_group.ends_with("_outro")
    {
        clear_animation(player, animator, actor_state, &jump_anim_group.clone());
    }

    // If we are in the air (e.g., falling), a jump animation should be playing.
    // If one isn't, this signals a logic error elsewhere, but this function's primary
    // responsibility is handling the discrete flags. We can check if a jump is active.
    if final_jump_state == JumpingStateKind::InAir {
        return actor_state
            .jumping
            .current_animation_group
            .starts_with("jump");
    }

    // No jump/land event was handled.
    false
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
            // Landing is a movement priority, Jump is a higher priority.
            priorities: [if is_landing {
                Priority::Movement
            } else {
                Priority::Jump
            }; NUM_DISCRETE_REGIONS],
            blend_mask: BlendMask::ALL.bits(),
            animation_speed: 1.0,
        };

        play_animation(player, animator, actor_state, anim_name, new_animation, 1.0);

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
    player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    actor_state: &mut ActorState,
    is_player: bool,
) {
    let weapon_type = actor_state.combat.weapon_type;
    let initial_move_state = actor_state.movement.state;

    // Determine the intended animation state, applying special logic to let turn animations finish smoothly.
    let effective_move_state =
        determine_effective_move_state(time, anim_clips, player, animator, actor_state, is_player);

    // Find the best animation clip for the determined state.
    let target_anim_name = resolve_movement_anim_name(effective_move_state, weapon_type, animator)
        .or_else(|| resolve_idle_anim_name(effective_move_state, weapon_type, animator));

    let new_name = target_anim_name.as_deref().unwrap_or("").to_string();
    let current_name = &actor_state.movement.current_animation_group.clone();

    // Check if a state change is actually needed.
    let is_stable_state = {
        let is_letting_outro_finish = current_name.ends_with("_outro")
            && matches!(
                initial_move_state,
                CharacterState::Idle
                    | CharacterState::IdleSneak
                    | CharacterState::IdleSwim
                    | CharacterState::TurnLeft
                    | CharacterState::TurnRight
                    | CharacterState::SwimTurnLeft
                    | CharacterState::SwimTurnRight
            );

        if is_letting_outro_finish {
            true
        } else if *current_name == new_name {
            true
        } else if let Some(target_def) = animator.animation_definitions.get(&new_name) {
            target_def.next_clip_name.as_deref() == Some(current_name)
        } else {
            false
        }
    };

    if is_stable_state && !new_name.is_empty() {
        actor_state.movement.last_state = effective_move_state;
        return;
    }

    // If the state is not stable, we need to switch animations.
    clear_animation(player, animator, actor_state, current_name);

    if !new_name.is_empty() {
        if let Some(definition) = animator.animation_definitions.get(&new_name) {
            let speed_multiplier = calculate_anim_speed_multiplier(
                effective_move_state,
                actor_state.is_running,
                actor_state.is_sneaking,
                definition,
                &animator,
                weapon_type,
                get_current_speed(&actor_state),
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
                player,
                animator,
                actor_state,
                &new_name,
                new_animation,
                speed_multiplier,
            );
        }
    }

    actor_state.movement.current_animation_group = new_name.to_string();
    actor_state.movement.last_state = effective_move_state;
}

/// Determines the character's animation state, preserving turning animations
/// for a short duration
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

    if is_player && is_currently_turning && new_state_is_idle {
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
                        let progress = if total_turn_duration > 0.0 {
                            total_elapsed_time / total_turn_duration
                        } else {
                            1.0
                        };
                        if progress < 0.4 {
                            return last_frame_state; // Override state to continue turning.
                        }
                    }
                }
            }
        }
    } else if !is_player {
        if actor_state.movement.turn_animation_timer > 0.0 {
            actor_state.movement.turn_animation_timer -= time.delta_secs();
        }
        if matches!(
            initial_move_state,
            CharacterState::TurnLeft | CharacterState::TurnRight
        ) {
            actor_state.movement.turn_animation_timer = 0.05;
        } else if is_currently_turning
            && new_state_is_idle
            && actor_state.movement.turn_animation_timer > 0.0
        {
            return last_frame_state; // Override state to continue turning.
        }
    }

    // In all other cases, the effective state is just the current input state.
    initial_move_state
}

/// Stops an animation and cleans up all related state from the controllers.
///
/// This function is essential for correctness. It ensures that when an animation group
/// (e.g., "walkforward") is stopped, its state is correctly removed from:
/// 1.  Bevy's `AnimationPlayer` (to stop the visual playback).
/// 2.  The custom `NifAnimator` (to free up animation regions for other priorities).
/// 3.  The main `ActorState` component (to prevent stale state bugs).
///
/// Returns the `ActiveAnimation` struct of the removed animation if it existed.
fn clear_animation(
    player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    actor_state: &mut ActorState,
    group_name: &str,
) -> Option<ActiveAnimation> {
    if group_name.is_empty() {
        return None;
    }

    // Remove the animation from our custom animator's tracking map.
    if let Some(removed_anim) = animator.active_animations.remove(group_name) {
        log::trace!("Stopping animation group: '{}'", group_name);

        // Stop the actual playback in Bevy's animation system.
        if let Some(definition) = animator.animation_definitions.get(group_name) {
            player.stop(definition.node_index);
        }

        // Free up the animation priority regions this animation was using.
        let blend_mask = BlendMask::from_bits_truncate(removed_anim.blend_mask);
        for i in 0..NUM_DISCRETE_REGIONS {
            let region_mask = BlendMask::from_bits_truncate(1 << i);
            if blend_mask.contains(region_mask) {
                if animator.active_regions[i] == removed_anim.priorities[i] {
                    animator.active_regions[i] = Priority::Idle;
                }
            }
        }

        // Prevent stale state by clearing the tracking fields in ActorState.
        // If this is skipped, the system might believe an animation is still playing
        // after it has been stopped, leading to incorrect logic on subsequent frames.
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
/// Correctly plays a new animation, stopping any lower-priority animations that
/// conflict with it, and correctly sets the looping behavior.
fn play_animation(
    player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    actor_state: &mut ActorState,
    group_key: &str,
    new_animation: ActiveAnimation,
    speed_multiplier: f32,
) {
    let new_blend_mask = BlendMask::from_bits_truncate(new_animation.blend_mask);
    let mut keys_to_remove = Vec::new();

    // This priority logic correctly finds and removes conflicting lower-priority animations.
    'outer: for (existing_key, existing_anim) in &animator.active_animations {
        let existing_blend_mask = BlendMask::from_bits_truncate(existing_anim.blend_mask);

        if !new_blend_mask.intersects(existing_blend_mask) {
            continue;
        }

        for i in 0..NUM_DISCRETE_REGIONS {
            let region_mask = BlendMask::from_bits_truncate(1 << i);
            let affects_new = new_blend_mask.contains(region_mask);
            let affects_existing = existing_blend_mask.contains(region_mask);

            if affects_new && affects_existing {
                if new_animation.priorities[i] >= existing_anim.priorities[i] {
                    keys_to_remove.push(existing_key.clone());
                    continue 'outer;
                }
            }
        }
    }

    for key in keys_to_remove {
        clear_animation(player, animator, actor_state, &key);
    }

    // Update the animator state with the new winning priorities.
    for i in 0..NUM_DISCRETE_REGIONS {
        let region_mask = BlendMask::from_bits_truncate(1 << i);
        if new_blend_mask.contains(region_mask) {
            animator.active_regions[i] = new_animation.priorities[i];
        }
    }

    let node_index = new_animation.node_index;

    // Add the animation to our tracking map.
    animator
        .active_animations
        .insert(group_key.to_string(), new_animation.clone());

    // Start the animation in Bevy.
    let anim_state = player.play(node_index).set_speed(speed_multiplier);

    // --- THIS IS THE FIX ---
    // We now check the loop_count from the ActiveAnimation struct.
    // An intro animation like "jumphh" will have a loop_count of 1.
    // A looping animation like "walkforward_loop" will have a loop_count of u32::MAX.
    if new_animation.loop_count == u32::MAX {
        // Only animations meant to loop forever will be repeated.
        anim_state.repeat();
    } else {
        // Animations with a loop_count of 0 or 1 will play once and then stop,
        // holding on their last frame. This is the correct behavior for intros and outros.
        anim_state.set_repeat(bevy::animation::RepeatAnimation::Never);
    }

    println!(
        "Playing anim '{}' with speed {:.2} (Looping: {})",
        group_key,
        speed_multiplier,
        new_animation.loop_count == u32::MAX
    );
}
/// Resolves the JUMP INTRO animation group name (e.g., "jumphh").
fn resolve_jump_intro_name(weapon_type: i32, animator: &NifAnimator) -> Option<String> {
    let base_group = "jump";
    let (specific_suffix, fallback_suffix) = get_weapon_suffixes(weapon_type);

    let mut candidates = Vec::with_capacity(3);
    if let Some(s) = specific_suffix {
        candidates.push(format!("{}{}", base_group, s));
    }
    if let Some(s) = fallback_suffix {
        if specific_suffix != fallback_suffix {
            candidates.push(format!("{}{}", base_group, s));
        }
    }
    candidates.push(base_group.to_string());

    for name in candidates.iter() {
        if animator.animation_definitions.contains_key(name) {
            return Some(name.clone());
        }
    }
    None
}

/// Resolves the JUMP LANDING animation group name (e.g., "jumphh_outro").
fn resolve_jump_landing_name(weapon_type: i32, animator: &NifAnimator) -> Option<String> {
    let base_prefix = "jump";
    let (specific_weapon_suffix, fallback_weapon_suffix) = get_weapon_suffixes(weapon_type);

    let mut base_candidates = Vec::with_capacity(3);
    if let Some(s) = specific_weapon_suffix {
        base_candidates.push(format!("{}{}", base_prefix, s));
    }
    if let Some(s) = fallback_weapon_suffix {
        if specific_weapon_suffix != fallback_weapon_suffix {
            base_candidates.push(format!("{}{}", base_prefix, s));
        }
    }
    base_candidates.push(base_prefix.to_string());

    for base in base_candidates.iter() {
        let outro_name = format!("{}_outro", base);
        if animator.animation_definitions.contains_key(&outro_name) {
            return Some(outro_name);
        }
    }
    None
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

    let (specific_suffix, fallback_suffix) = get_weapon_suffixes(weapon_type);
    let mut candidates = Vec::with_capacity(3);

    if let Some(suffix) = specific_suffix {
        candidates.push(format!("{}{}", base_group, suffix));
    }
    if let Some(suffix) = fallback_suffix {
        if specific_suffix != fallback_suffix {
            candidates.push(format!("{}{}", base_group, suffix));
        }
    }
    candidates.push(base_group.to_string());

    for name in candidates {
        if animator.animation_definitions.contains_key(&name) {
            return Some(name);
        }
    }
    None
}
/// Resolves a logical movement state into a final, playable animation group name.
///
/// This function encapsulates the complex fallback logic from OpenMW:
/// 1. Maps a CharacterState enum to a base animation name (e.g., "walkforward").
/// 2. Handles the "swim" to "walk/run" fallback if no swim animations exist.
/// 3. Appends the correct weapon suffix (e.g., "1h", "hh").
/// 4. Checks if the specific animation exists, falling back to the non-suffixed version if needed.
///
/// # Returns
/// An `Option<String>` containing the name of the animation to play, or `None` if no
/// valid animation could be found for the given state.

/// Resolves a logical movement state into a final, playable animation group name.
///
/// This function fully implements the complex fallback logic from OpenMW's
/// `CharacterController::refreshMovementAnims`. The fallback order is:
/// 1. Try the most specific animation (e.g., "walkforward" + "axe").
/// 2. If that fails, try a more generic weapon fallback (e.g., "walkforward" + "2h").
/// 3. If that fails, try the base animation with no weapon suffix (e.g., "walkforward").
/// 4. If the initial state was a "run" state and all of the above failed, repeat steps 1-3
///    using the "walk" equivalent of the state.
/// 5. If a "swim" animation is requested but none exists, it will automatically fall back
///    to the land-based equivalent.
fn resolve_movement_anim_name(
    state: CharacterState,
    weapon_type: i32,
    animator: &NifAnimator,
) -> Option<String> {
    // --- Step 1: Map the CharacterState enum to a base string name ---
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
        _ => return None, // Not a standard movement state
    };

    // --- Step 2: Handle the swim-to-walk/run fallback ---
    let is_swim = base_group.starts_with("swim");
    if is_swim
        && !animator
            .animation_definitions
            .keys()
            .any(|k| k.starts_with(base_group))
    {
        base_group = base_group.strip_prefix("swim").unwrap_or(base_group);
    }

    // --- Step 3: Build a list of candidate animation names in order of priority ---
    let (specific_suffix, fallback_suffix) = get_weapon_suffixes(weapon_type);
    let mut candidates = Vec::with_capacity(6);

    // Primary candidates (using the original state, e.g., "runforward")
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

    // --- Step 4: Add run-to-walk fallbacks if necessary ---
    if base_group.starts_with("run") {
        if let Some(walk_base) = base_group.strip_prefix("run") {
            let walk_group = format!("walk{}", walk_base);
            if let Some(suffix) = specific_suffix {
                candidates.push(format!("{}{}", walk_group, suffix));
            }
            if let Some(suffix) = fallback_suffix {
                if specific_suffix != fallback_suffix {
                    candidates.push(format!("{}{}", walk_group, suffix));
                }
            }
            candidates.push(walk_group);
        }
    }

    // --- Step 5: Find the first valid candidate ---
    for name in candidates {
        if animator.animation_definitions.contains_key(&name) {
            return Some(name);
        }
    }

    // No suitable animation was found at all.
    None
}
/// Returns the specific and fallback animation suffixes for a given weapon type.
///
/// This function replicates the logic from OpenMW's `getWeaponShortGroup` and
/// `fallbackShortWeaponGroup`. It provides two suffixes to allow for a robust
/// fallback system when resolving animation names.
///
/// # Arguments
/// * `weapon_type` - An integer representing the `ESM::Weapon::Type`.
///
/// # Returns
/// A tuple `(specific_suffix, fallback_suffix)`.
/// - `specific_suffix`: The ideal suffix for this weapon (e.g., "axe", "longbow").
/// - `fallback_suffix`: A more generic suffix to try if the specific one fails (e.g., "1h", "bow").
fn get_weapon_suffixes(weapon_type: i32) -> (Option<&'static str>, Option<&'static str>) {
    // This mapping is based on OpenMW's `data/animation.cpp` or similar definitions.
    // The integer values correspond to the ESM::Weapon::Type enum.
    match weapon_type {
        0 => (Some("hh"), None), // Hand-to-hand
        1 => (Some("shortblade"), Some("1h")),
        2 => (Some("longblade"), Some("1h")),
        3 => (Some("axe"), Some("1h")),
        4 => (Some("blunt"), Some("1h")),
        5 => (Some("longblade"), Some("2h")), // Longblade Two-Hand (Claymores)
        6 => (Some("axe"), Some("2h")),       // Axe Two-Hand
        7 => (Some("blunt"), Some("2h")),     // Blunt Two-Hand
        8 => (Some("spear"), Some("2h")),
        9 => (Some("bow"), Some("bow")), // Marksman Bow uses its own suffix
        10 => (Some("crossbow"), Some("crossbow")),
        11 => (Some("throwing"), Some("throwing")),
        12 => (Some("spell"), None), // Spells
        // In OpenMW, crossbows use 1h movement animations.
        13 => (Some("crossbow"), Some("1h")),
        _ => (None, None), // No weapon or unknown
    }
}
/// Converts a running state to its walking equivalent, mirroring OpenMW's logic.
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
        // For any other state (including walk, idle, turn, etc.), return it as-is.
        _ => state,
    }
}

/// Calculates the correct animation speed multiplier, encapsulating all of OpenMW's nuanced rules.
fn calculate_anim_speed_multiplier(
    state: CharacterState,
    is_running: bool,
    is_sneaking: bool,
    definition: &AnimationDefinition,
    animator: &NifAnimator,
    weapon_type: i32,
    character_speed: f32,
) -> f32 {
    // For non-linear movement states, the speed is always 1.0.
    if matches!(
        state,
        CharacterState::Idle | CharacterState::TurnLeft | CharacterState::TurnRight
    ) {
        return 1.0;
    }

    let mut anim_base_speed;

    // --- The "Use Walk Speed for Running" Rule ---
    // For running states, OpenMW uses the 'walk' animation's speed as the baseline for calculation.
    if is_running {
        let walk_state = run_state_to_walk_state(state);
        // We must resolve the full animation name for the walk state (e.g., "walkforwardhh")
        if let Some(walk_anim_name) = resolve_movement_anim_name(walk_state, weapon_type, animator)
        {
            if let Some(walk_def) = animator.animation_definitions.get(&walk_anim_name) {
                anim_base_speed = walk_def.base_velocity.length();
            } else {
                // Fallback if the specific walk animation doesn't exist for some reason
                anim_base_speed = definition.base_velocity.length();
            }
        } else {
            anim_base_speed = definition.base_velocity.length();
        }
    } else {
        // If not running, just use the current animation's base velocity.
        anim_base_speed = definition.base_velocity.length();
    }

    // --- The "Hardcoded Fallback" Rule ---
    // If the calculated speed from the animation data is too low to be reliable...
    if anim_base_speed <= 1.0 {
        // ...substitute it with a hardcoded default value based on the current posture.
        anim_base_speed = if is_sneaking {
            33.5452
        } else if is_running {
            222.857
        } else {
            154.064
        };
    }

    // --- Final Calculation ---
    let final_anim_speed = anim_base_speed / MW_SCALE_REDUCTION;
    if final_anim_speed > 1e-4 {
        character_speed / final_anim_speed
    } else {
        1.0
    }
}
