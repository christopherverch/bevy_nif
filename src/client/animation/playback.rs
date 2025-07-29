use bevy::animation::AnimationPlayer;
use bevy::{log, prelude::*};
use bevy_nif::nif_animation::bevy_types::{ActiveAnimation, Priority};
use bevy_nif::nif_animation::{NUM_DISCRETE_REGIONS, NifAnimator};
use tes3mp_rust::actor_defs::ActorState;
pub struct AnimationContext<'w> {
    pub animation_player: &'w mut AnimationPlayer,
    pub nif_animator: &'w mut NifAnimator,
    pub actor_state: &'w mut ActorState,
    pub anim_graphs: &'w mut Assets<AnimationGraph>,
    pub graph_handle: &'w AnimationGraphHandle,
}
impl<'w> AnimationContext<'w> {
    /// Plays a new animation and recalculates all priority-based masks.
    /// Plays a new animation and recalculates all priority-based masks.
    pub fn play_animation(
        &mut self,
        group_key: &str,
        new_animation: ActiveAnimation,
        start_time: Option<f32>,
    ) {
        // Stop any animation that is an intro to this one.
        if let Some(base_name) = group_key.strip_suffix("_loop") {
            self.clear_animation(base_name);
        }

        let node_index = new_animation.node_index;
        let speed_multiplier = new_animation.speed_mult;

        // Add the new animation to the active list. Its blend_mask is its original, intended scope.
        self.nif_animator
            .active_animations
            .insert(group_key.to_string(), new_animation.clone());

        // Recalculate and apply masks for ALL active animations.
        self.recalculate_and_apply_masks();

        // Start playback.
        let anim_state = self
            .animation_player
            .play(node_index)
            .set_speed(speed_multiplier);
        if let Some(time) = start_time {
            anim_state.seek_to(time);
        }

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
    pub fn clear_animation(&mut self, group_name: &str) -> Option<ActiveAnimation> {
        if group_name.is_empty() {
            return None;
        }

        if let Some(removed_anim) = self.nif_animator.active_animations.remove(group_name) {
            log::trace!("Stopping animation group: '{}'", group_name);

            // Stop playback in Bevy
            if let Some(definition) = self.nif_animator.animation_definitions.get(group_name) {
                self.animation_player.stop(definition.node_index);
                if let Some(graph) = self.anim_graphs.get_mut(self.graph_handle) {
                    if let Some(node) = graph.get_mut(definition.node_index) {
                        node.mask = !0; // Mask all bones for this node
                    }
                }
            }

            // After removing, recalculate and apply masks for all remaining animations.
            self.recalculate_and_apply_masks();

            // Prevent stale state by clearing tracking fields in ActorState.
            if self.actor_state.jumping.current_animation_group == group_name {
                self.actor_state.jumping.current_animation_group.clear();
            }
            if self.actor_state.movement.current_animation_group == group_name {
                self.actor_state.movement.current_animation_group.clear();
            }
            if self.actor_state.combat.current_animation_group == group_name {
                self.actor_state.combat.current_animation_group.clear();
            }
            return Some(removed_anim);
        }
        None
    }

    /// Clears movement, jumping, and combat animations
    pub fn clear_all_other_animations(&mut self) {
        self.clear_animation(&self.actor_state.movement.current_animation_group.clone());
        self.clear_animation(&self.actor_state.jumping.current_animation_group.clone());
        self.clear_animation(&self.actor_state.combat.current_animation_group.clone());
    }

    /// Recalculates the effective blend mask for every active animation based on priority and applies it.
    pub fn recalculate_and_apply_masks(&mut self) {
        let mut effective_masks: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();

        // For each body region, find the animation with the highest priority that *wants* to affect it.
        for i in 0..NUM_DISCRETE_REGIONS {
            let region_mask = 1 << i;
            let mut winner_key: Option<String> = None;
            let mut highest_priority = Priority::Idle;

            for (key, anim) in &self.nif_animator.active_animations {
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
        if let Some(graph) = self.anim_graphs.get_mut(self.graph_handle) {
            for (key, _anim) in &mut self.nif_animator.active_animations {
                let effective_mask = effective_masks.get(key).cloned().unwrap_or(0);

                // Update visual mask in the animation graph
                if let Some(definition) = self.nif_animator.animation_definitions.get(key) {
                    if let Some(node) = graph.get_mut(definition.node_index) {
                        node.mask = !effective_mask; // The graph mask is an UNMASK
                    }
                }
            }
        }
    }
}
