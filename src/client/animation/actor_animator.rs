use bevy::{
    animation::{
        AnimationClip, AnimationPlayer,
        graph::{AnimationGraph, AnimationGraphHandle},
    },
    asset::Assets,
    log::{self, warn},
};
use bevy_ecs::system::{Query, Res, ResMut};
use bevy_nif::nif_animation::{
    AnimationDefinition, BlendMask, NUM_DISCRETE_REGIONS, NifAnimator,
    bevy_types::{ActiveAnimation, Priority},
};

use tes3mp_rust::{
    actor_defs::{ActorState, CharacterState, JumpingStateKind, UpperBodyState},
    defs::{CreatureStats, IdToEntityMap, PlayerInput},
};

use crate::{actor::actor_funcs::get_current_speed, const_defs::MW_SCALE_REDUCTION};

use super::animation_helpers::get_anim_components_for_creature;

/// Checks all state variables of the actor and decides how they should be animated for this frame
/// Also calls functions to handle looping, stopping, etc of animations
pub fn animate_actors_using_state(
    anim_clips: Res<Assets<AnimationClip>>,
    mut actor_query: Query<(&CreatureStats, &mut ActorState)>,
    mut animator_q: Query<(
        &mut AnimationPlayer,
        &mut NifAnimator,
        &AnimationGraphHandle,
    )>,
    player_input: Res<PlayerInput>,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    id_to_entity_map: Res<IdToEntityMap>,
) {
    for (creature_stats, mut actor_state) in actor_query.iter_mut() {
        let Some((_, mut animation_player, mut animator, graph_handle)) =
            get_anim_components_for_creature(&creature_stats, &id_to_entity_map, &mut animator_q)
        else {
            continue;
        };
        let vitals_took_control = handle_vitals_animations(
            &mut animation_player,
            &mut animator,
            &mut anim_graphs,
            graph_handle,
            &mut actor_state,
        );

        if vitals_took_control {
            continue; // Vitals logic is handling animations, skip everything else.
        }
        // First, process any ongoing transitions from the previous frame.
        handle_animation_transitions(
            &mut animation_player,
            &mut animator,
            &anim_clips,
            &mut anim_graphs,
            graph_handle,
            &mut actor_state,
        );
        // Handle any high-priority combat actions like equipping.
        // This function will set an upper-body animation if needed.
        handle_actor_weapon_actions(
            &mut animation_player,
            &mut animator,
            &mut anim_graphs,
            graph_handle,
            &player_input,
            &mut actor_state,
        );

        // Next, handle all locomotion (jumping or ground movement).
        // The resolver has already ensured that only one of these can be active.
        handle_actor_jumping(
            &mut animation_player,
            &mut animator,
            &mut anim_graphs,
            graph_handle,
            &mut actor_state,
        );

        handle_actor_ground_movement(
            &mut animation_player,
            &mut anim_graphs,
            graph_handle,
            &mut animator,
            &mut actor_state,
        );
    }
}

/// Checks for finished non-looping animations and processes their transitions or removals.
fn handle_animation_transitions(
    animation_player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    anim_clips: &Assets<AnimationClip>,
    anim_graphs: &mut Assets<AnimationGraph>,
    graph_handle: &AnimationGraphHandle,
    actor_state: &mut ActorState,
) {
    let mut transitions_to_perform = Vec::new();
    let mut animations_to_remove = Vec::new();

    // Step 1: Identify finished animations.
    for (group_name, active_anim) in animator.active_animations.iter() {
        if active_anim.loop_count == u32::MAX {
            continue;
        }
        if let Some(anim_state) = animation_player.animation(active_anim.node_index) {
            if let Some(clip) = anim_clips.get(&active_anim.clip_handle) {
                let remaining_time = clip.duration() / anim_state.speed() - anim_state.elapsed();
                if remaining_time <= 0.001 {
                    if let Some(next_clip_name) = &active_anim.next_clip_name {
                        transitions_to_perform.push((group_name.clone(), next_clip_name.clone()));
                    } else if active_anim.loop_count == 0 {
                        animations_to_remove.push(group_name.clone());
                    }
                }
            }
        }
    }

    // Step 2: Execute state changes for finished animations.
    for name in animations_to_remove {
        let upper_body_state = actor_state.combat.upper_body_state;

        // Using the state is more robust than matching the string name
        if matches!(upper_body_state, UpperBodyState::Equipping) {
            actor_state.combat.upper_body_state = UpperBodyState::WeaponEquipped;

            // --- THIS IS THE FIX ---
            // Find the currently active movement animation.
            let movement_group = &actor_state.movement.current_animation_group;
            if !movement_group.is_empty() {
                if let Some(active_move_anim) = animator.active_animations.get_mut(movement_group) {
                    // Update its mask to reclaim the full body.
                    active_move_anim.blend_mask = BlendMask::ALL;
                }
            }
        } else if matches!(upper_body_state, UpperBodyState::Unequipping) {
            actor_state.combat.upper_body_state = UpperBodyState::None;
            actor_state.combat.weapon_type = 0;
        } else if matches!(
            upper_body_state,
            UpperBodyState::AttackWindUp
                | UpperBodyState::AttackRelease
                | UpperBodyState::AttackEnd
        ) {
            actor_state.combat.upper_body_state = UpperBodyState::WeaponEquipped;
        }

        // Now, remove the animation that just finished.
        clear_animation(
            animation_player,
            animator,
            actor_state,
            anim_graphs,
            graph_handle,
            &name,
        );

        // The special case for unequipping is still needed to force a change
        // from a weapon-specific animation (e.g., "runforward1h") to a generic one ("runforward").
        if upper_body_state == UpperBodyState::Unequipping {
            clear_animation(
                animation_player,
                animator,
                actor_state,
                anim_graphs,
                graph_handle,
                &actor_state.movement.current_animation_group.clone(),
            );
        }
    }

    // Step 3: Handle intro-to-loop transitions. (No change here)
    for (current_anim_name, next_anim_name) in transitions_to_perform {
        if let Some(mut transitioning_anim) = clear_animation(
            animation_player,
            animator,
            actor_state,
            anim_graphs,
            graph_handle,
            &current_anim_name,
        ) {
            if let Some(definition) = animator.animation_definitions.get(&next_anim_name) {
                let original_priority = transitioning_anim.priorities[0];
                transitioning_anim.clip_handle = definition.clip_handle.clone();
                transitioning_anim.node_index = definition.node_index;
                transitioning_anim.next_clip_name = None;
                transitioning_anim.loop_count = u32::MAX;
                play_animation(
                    animation_player,
                    animator,
                    actor_state,
                    anim_graphs,
                    graph_handle,
                    &next_anim_name,
                    transitioning_anim,
                );
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
    anim_graphs: &mut Assets<AnimationGraph>,
    graph_handle: &AnimationGraphHandle,
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
                anim_graphs,
                graph_handle,
                &actor_state.movement.current_animation_group.clone(),
            );
            clear_animation(
                animation_player,
                animator,
                actor_state,
                anim_graphs,
                graph_handle,
                &actor_state.jumping.current_animation_group.clone(),
            );
            play_jump_animation(
                animation_player,
                animator,
                anim_graphs,
                graph_handle,
                actor_state,
                &name,
                false,
            );
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
                anim_graphs,
                graph_handle,
                &actor_state.movement.current_animation_group.clone(),
            );
            clear_animation(
                animation_player,
                animator,
                actor_state,
                anim_graphs,
                graph_handle,
                &actor_state.jumping.current_animation_group.clone(),
            );
            play_jump_animation(
                animation_player,
                animator,
                anim_graphs,
                graph_handle,
                actor_state,
                &name,
                true,
            );
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
            anim_graphs,
            graph_handle,
            &actor_state.movement.current_animation_group.clone(),
        );

        if let Some(name) = resolve_jump_intro_name(actor_state.combat.weapon_type, animator) {
            play_jump_animation(
                animation_player,
                animator,
                anim_graphs,
                graph_handle,
                actor_state,
                &name,
                false,
            );
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
            anim_graphs,
            graph_handle,
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
    anim_graphs: &mut Assets<AnimationGraph>,
    graph_handle: &AnimationGraphHandle,
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
            blend_mask: BlendMask::ALL,
            speed_mult: 1.0,
        };

        play_animation(
            player,
            animator,
            actor_state,
            anim_graphs,
            graph_handle,
            anim_name,
            new_animation,
        );

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
    animation_player: &mut AnimationPlayer,
    anim_graphs: &mut Assets<AnimationGraph>,
    graph_handle: &AnimationGraphHandle,
    animator: &mut NifAnimator,
    actor_state: &mut ActorState,
) {
    let effective_move_state = actor_state.movement.state;

    let target_anim_name = resolve_movement_anim_name(effective_move_state, actor_state, animator)
        .or_else(|| resolve_idle_anim_name(effective_move_state, actor_state, animator));

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
    clear_animation(
        animation_player,
        animator,
        actor_state,
        anim_graphs,
        graph_handle,
        current_name,
    );

    if !new_name.is_empty() {
        if let Some(definition) = animator.animation_definitions.get(&new_name) {
            let speed_multiplier = calculate_anim_speed_multiplier(
                effective_move_state,
                actor_state.is_running,
                actor_state.is_sneaking,
                definition,
                animator,
                actor_state,
                get_current_speed(actor_state),
            );
            let blend_mask = if matches!(
                actor_state.combat.upper_body_state,
                UpperBodyState::Equipping | UpperBodyState::Unequipping
            ) {
                BlendMask::LOWER_BODY // Only affect legs if using a weapon
            } else {
                BlendMask::ALL // Affect the whole body otherwise
            };
            // Idle animations must have Idle priority, not Movement priority.
            let priority = if new_name.starts_with("idle") {
                Priority::Idle
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

            play_animation(
                animation_player,
                animator,
                actor_state,
                anim_graphs,
                graph_handle,
                &new_name,
                new_animation,
            );
        }
    }

    actor_state.movement.current_animation_group = new_name;
    actor_state.movement.last_state = effective_move_state;
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
/// Stops an animation and triggers a recalculation of all priority-based masks.
fn clear_animation(
    animation_player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    actor_state: &mut ActorState,
    anim_graphs: &mut Assets<AnimationGraph>,
    graph_handle: &AnimationGraphHandle,
    group_name: &str,
) -> Option<ActiveAnimation> {
    if group_name.is_empty() {
        return None;
    }

    if let Some(removed_anim) = animator.active_animations.remove(group_name) {
        log::trace!("Stopping animation group: '{}'", group_name);

        // Stop playback in Bevy
        if let Some(definition) = animator.animation_definitions.get(group_name) {
            animation_player.stop(definition.node_index);
            if let Some(graph) = anim_graphs.get_mut(graph_handle) {
                if let Some(node) = graph.get_mut(definition.node_index) {
                    node.mask = !0; // Mask all bones for this node
                }
            }
        }

        // After removing, recalculate and apply masks for all remaining animations.
        recalculate_and_apply_masks(animator, anim_graphs, graph_handle);

        // Prevent stale state by clearing tracking fields in ActorState.
        if actor_state.jumping.current_animation_group == group_name {
            actor_state.jumping.current_animation_group.clear();
        }
        if actor_state.movement.current_animation_group == group_name {
            actor_state.movement.current_animation_group.clear();
        }
        if actor_state.combat.current_animation_group == group_name {
            actor_state.combat.current_animation_group.clear();
        }
        return Some(removed_anim);
    }
    None
}

/// Plays a new animation and recalculates all priority-based masks.
fn play_animation(
    animation_player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    actor_state: &mut ActorState,
    anim_graphs: &mut Assets<AnimationGraph>,
    graph_handle: &AnimationGraphHandle,
    group_key: &str,
    new_animation: ActiveAnimation,
) {
    // Stop any animation that is an intro to this one.
    if let Some(base_name) = group_key.strip_suffix("_loop") {
        clear_animation(
            animation_player,
            animator,
            actor_state,
            anim_graphs,
            graph_handle,
            base_name,
        );
    }

    let node_index = new_animation.node_index;
    let speed_multiplier = new_animation.speed_mult;

    // Add the new animation to the active list. Its blend_mask is its original, intended scope.
    animator
        .active_animations
        .insert(group_key.to_string(), new_animation.clone());

    // Recalculate and apply masks for ALL active animations.
    recalculate_and_apply_masks(animator, anim_graphs, graph_handle);

    // Start playback.
    let anim_state = animation_player
        .play(node_index)
        .set_speed(speed_multiplier);
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
    actor_state: &ActorState,
    animator: &NifAnimator,
) -> Option<String> {
    let base_group = match state {
        CharacterState::Idle => "idle",
        CharacterState::IdleSneak => "idlesneak",
        CharacterState::IdleSwim => "idleswim",
        _ => return None, // Not an idle state
    };
    // Only search for weapon-specific idles if a weapon is actually equipped.
    if actor_state.combat.weapon_type > 0 {
        let candidates =
            generate_weapon_anim_candidates(base_group, actor_state.combat.weapon_type);
        if let Some(found) = find_first_valid_animation(&candidates, animator) {
            return Some(found);
        }
    }

    // Fallback to the generic base idle animation.
    Some(base_group.to_string())
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
pub fn resolve_movement_anim_name(
    state: CharacterState,
    actor_state: &ActorState,
    animator: &NifAnimator,
) -> Option<String> {
    // The state is WeaponEquipped, not Equipping or Unequipping.
    let weapon_equipped = actor_state.combat.weapon_type > 0;
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

    // If a weapon is fully equipped, try to find a weapon-specific movement animation.
    if weapon_equipped {
        let candidates =
            generate_weapon_anim_candidates(base_group, actor_state.combat.weapon_type);
        if let Some(found) = find_first_valid_animation(&candidates, animator) {
            return Some(found);
        }
    }

    // Fallback for swim animations (try non-swim version if specific swim anim not found)
    if base_group.starts_with("swim")
        && !animator
            .animation_definitions
            .keys()
            .any(|k| k.starts_with(base_group))
    {
        base_group = base_group.strip_prefix("swim").unwrap_or(base_group);
    }

    // Fallback for run animations (try walk version if run anim not found)
    if base_group.starts_with("run") {
        if find_first_valid_animation(&[base_group.to_string()], animator).is_none() {
            if let Some(walk_base) = base_group.strip_prefix("run") {
                let walk_group = format!("walk{}", walk_base);
                return Some(walk_group);
            }
        }
    }

    Some(base_group.to_string())
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
    actor_state: &ActorState,
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
        anim_base_speed = resolve_movement_anim_name(walk_state, &actor_state, animator)
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
    anim_graphs: &mut Assets<AnimationGraph>,
    graph_handle: &AnimationGraphHandle,
    player_input: &PlayerInput,
    actor_state: &mut ActorState,
) -> bool {
    // This mirrors OpenMW's check for releasing the attack button.
    if actor_state.combat.upper_body_state == UpperBodyState::AttackWindUp
        && !player_input.attack_is_held
    {
        // For the simple attack animations, we transition directly to AttackEnd
        // to play the follow-through and then reset.
        actor_state.combat.upper_body_state = UpperBodyState::AttackEnd;
    }

    let weapon_group = get_weapon_long_group(actor_state.combat.weapon_type);
    if weapon_group.is_empty() {
        return false;
    }

    let (action, loop_count) = match actor_state.combat.upper_body_state {
        UpperBodyState::Equipping => ("equip", 0),
        UpperBodyState::Unequipping => ("unequip", 0),
        UpperBodyState::AttackWindUp => (actor_state.combat.attack_type.as_str(), 0),
        _ => return false,
    };

    let anim_name = if weapon_group.is_empty() {
        // Fallback for creatures or unexpected cases
        match action {
            "chop" => "attack1".to_string(),
            "slash" => "attack2".to_string(),
            "thrust" => "attack3".to_string(),
            _ => action.to_string(),
        }
    } else {
        // This is the correct logic for player/NPCs, combining the
        // weapon group and the attack type.
        format!("{}: {}", weapon_group, action)
    };
    if actor_state.combat.current_animation_group == anim_name {
        return true;
    }

    if animator.animation_definitions.contains_key(&anim_name) {
        // --- START OF FIX ---
        // Determine if the character is moving *before* we clear the movement animation state.
        let was_moving = !actor_state.movement.current_animation_group.is_empty()
            && !actor_state
                .movement
                .current_animation_group
                .starts_with("idle");

        // Force the ground movement to re-evaluate by clearing the current movement animation.
        clear_animation(
            animation_player,
            animator,
            actor_state,
            anim_graphs,
            graph_handle,
            &actor_state.movement.current_animation_group.clone(),
        );

        let definition = animator.animation_definitions.get(&anim_name).unwrap();

        // Use the `was_moving` state we captured earlier to set the correct mask.
        let blend_mask = if was_moving {
            BlendMask::UPPER_BODY
        } else {
            BlendMask::ALL
        };
        // --- END OF FIX ---

        let new_animation = ActiveAnimation {
            clip_handle: definition.clip_handle.clone(),
            node_index: definition.node_index,
            next_clip_name: None,
            loop_count: 0,
            speed_mult: 1.0,
            blend_mask,
            priorities: [
                Priority::Idle,   // Lower Body
                Priority::Weapon, // Torso
                Priority::Weapon, // Left Arm
                Priority::Weapon, // Right Arm
            ],
        };

        play_animation(
            animation_player,
            animator,
            actor_state,
            anim_graphs,
            graph_handle,
            &anim_name,
            new_animation,
        );
        actor_state.combat.current_animation_group = anim_name;
    } else {
        warn!("Could not find weapon animation definition: {}", anim_name);
        match actor_state.combat.upper_body_state {
            UpperBodyState::Equipping => {
                actor_state.combat.upper_body_state = UpperBodyState::WeaponEquipped
            }
            UpperBodyState::Unequipping => {
                actor_state.combat.upper_body_state = UpperBodyState::None
            }
            _ => {}
        }
        return false;
    }

    true
}
// Corresponds to the weapon's main animation group (e.g., "weapontwohand", "bow")
fn get_weapon_long_group(weapon_type: i32) -> &'static str {
    // Based on ESM::Weapon::Type
    match weapon_type {
        0 => "handtohand",
        1..=4 => "weapononehand",
        5..=7 => "weapontwohand", // For swords, axes, blunts
        8 => "weapontwowide",     // Spears use a different group
        9 => "bow",
        10 | 13 => "crossbow",
        11 => "throwweapon", // Corrected from "throwing" to match anim names
        12 => "spellcast",   // Corrected from "spell" to match anim names
        _ => "",
    }
}

/// Returns the specific and fallback animation suffixes for a given weapon type.
fn get_weapon_suffixes(weapon_type: i32) -> (Option<&'static str>, Option<&'static str>) {
    // These now match the suffixes found in your animation list (hh, 1h, 2c, 2w, etc.)
    match weapon_type {
        0 => (Some("hh"), None),     // Hand-to-hand
        1..=4 => (Some("1h"), None), // All 1-handed weapons use "1h"
        5..=7 => (Some("2c"), None), // 2-handed close-stance weapons (swords, axes)
        8 => (Some("2w"), None),     // 2-handed wide-stance weapons (spears)
        9 => (Some("bow"), None),
        10 | 13 => (Some("crossbow"), Some("1h")), // Crossbows use 1h for movement
        12 => (Some("spell"), None),               // Spellcasting uses "spell"
        _ => (None, None),                         // Thrown, Spell, and None have no suffix
    }
}
/// Recalculates the effective blend mask for every active animation based on priority and applies it.
fn recalculate_and_apply_masks(
    animator: &mut NifAnimator,
    anim_graphs: &mut Assets<AnimationGraph>,
    graph_handle: &AnimationGraphHandle,
) {
    let mut effective_masks: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();

    // For each body region, find the animation with the highest priority that *wants* to affect it.
    for i in 0..NUM_DISCRETE_REGIONS {
        let region_mask = 1 << i;
        let mut winner_key: Option<String> = None;
        let mut highest_priority = Priority::Idle;

        for (key, anim) in &animator.active_animations {
            // THIS IS THE KEY CHANGE: We now read the animation's *original*, intended blend mask.
            let intended_mask = anim.blend_mask;
            if intended_mask.bits() & region_mask != 0 {
                if anim.priorities[i] >= highest_priority {
                    highest_priority = anim.priorities[i];
                    winner_key = Some(key.clone());
                }
            }
        }

        // Grant this region to the winner.
        if let Some(key) = winner_key {
            *effective_masks.entry(key).or_insert(0) |= region_mask;
        }
    }

    // Apply the newly calculated effective masks to the visual animation graph.
    // We NO LONGER modify the `anim.blend_mask` field itself.
    if let Some(graph) = anim_graphs.get_mut(graph_handle) {
        for (key, anim) in &mut animator.active_animations {
            let effective_mask = effective_masks.get(key).cloned().unwrap_or(0);

            // Update visual mask in the animation graph
            if let Some(definition) = animator.animation_definitions.get(key) {
                if let Some(node) = graph.get_mut(definition.node_index) {
                    node.mask = !effective_mask; // The graph mask is an UNMASK
                }
            }
        }
    }
}

/// This function replicates OpenMW's `refreshHitRecoilAnims` functionality.
/// It plays the hit animation if the state is set, and clears the state
/// after the animation is finished.
fn handle_vitals_animations(
    animation_player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    anim_graphs: &mut Assets<AnimationGraph>,
    graph_handle: &AnimationGraphHandle,
    actor_state: &mut ActorState,
) -> bool {
    // Returns true if it took control

    // Check if a hit animation is currently playing
    if !actor_state.vitals.current_hit_animation.is_empty() {
        // If the animation is no longer playing, we can clear the state.
        if !animator
            .active_animations
            .contains_key(&actor_state.vitals.current_hit_animation)
        {
            actor_state.vitals.hit_state = CharacterState::None;
            actor_state.vitals.current_hit_animation.clear();
        }
    }

    // If no hit state is active, do nothing.
    if actor_state.vitals.hit_state == CharacterState::None {
        return false;
    }

    // A hit state IS active. Let's play the animation if it's not already playing.
    let anim_name = match actor_state.vitals.hit_state {
        CharacterState::Hit => "hit".to_string(),
        CharacterState::KnockDown => "knockdown".to_string(),
        // Add other hit states here later
        _ => return false,
    };

    if actor_state.vitals.current_hit_animation == anim_name {
        return true; // Animation is already playing, do nothing else.
    }

    // A new hit state was set, so we need to start the animation.
    // It must interrupt everything else.
    clear_all_other_animations(
        animation_player,
        animator,
        actor_state,
        anim_graphs,
        graph_handle,
    );

    if let Some(definition) = animator.animation_definitions.get(&anim_name) {
        let new_animation = ActiveAnimation {
            clip_handle: definition.clip_handle.clone(),
            node_index: definition.node_index,
            next_clip_name: None,
            loop_count: 0, // Play once.
            priorities: [Priority::Hit; NUM_DISCRETE_REGIONS],
            blend_mask: BlendMask::ALL,
            speed_mult: 1.0,
        };
        play_animation(
            animation_player,
            animator,
            actor_state,
            anim_graphs,
            graph_handle,
            &anim_name,
            new_animation,
        );
        actor_state.vitals.current_hit_animation = anim_name;
    }

    true // Vitals state took control.
}

// You will also need a helper to clear other animations.
fn clear_all_other_animations(
    animation_player: &mut AnimationPlayer,
    animator: &mut NifAnimator,
    actor_state: &mut ActorState,
    anim_graphs: &mut Assets<AnimationGraph>,
    graph_handle: &AnimationGraphHandle,
) {
    clear_animation(
        animation_player,
        animator,
        actor_state,
        anim_graphs,
        graph_handle,
        &actor_state.movement.current_animation_group.clone(),
    );
    clear_animation(
        animation_player,
        animator,
        actor_state,
        anim_graphs,
        graph_handle,
        &actor_state.jumping.current_animation_group.clone(),
    );
    clear_animation(
        animation_player,
        animator,
        actor_state,
        anim_graphs,
        graph_handle,
        &actor_state.combat.current_animation_group.clone(),
    );
}
