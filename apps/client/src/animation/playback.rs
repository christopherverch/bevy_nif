use bevy::animation::{AnimationEventTrigger, AnimationPlayer};
use bevy::{log, prelude::*};
use bevy_nif::nif_animation::bevy_types::{ActiveAnimation, Priority};
use bevy_nif::nif_animation::{BlendMask, NUM_DISCRETE_REGIONS, NifAnimator, NifEvent};
pub struct AnimationContext<'w> {
    pub animation_player: &'w mut AnimationPlayer,
    pub nif_animator: &'w mut NifAnimator,
    pub anim_graphs: &'w mut Assets<AnimationGraph>,
    pub graph_handle: &'w AnimationGraphHandle,
}
impl<'w> AnimationContext<'w> {
    pub fn has_animation(&self, key: &str) -> bool {
        self.nif_animator.animation_definitions.contains_key(key)
    }
    pub fn play_animation_2(
        &mut self,
        animations: Res<Assets<AnimationClip>>,
        group_name: &str,
        priorities: [Priority; 4],
        blend_mask: BlendMask,
        auto_disable: bool,
        speed_mult: f32,
        start_time: f32,
        loop_count: u32,
        commands: &mut Commands,
    ) {
        // Early return for invalid input
        if group_name.is_empty() {
            //   self.reset_active_groups();
            return;
        }
        // Already playing, just update the priority
        let mut animation_already_exists = false;
        if let Some(anim) = self.nif_animator.active_animations.get_mut(group_name) {
            anim.priorities = priorities;
            animation_already_exists = true;
        }
        let mut animation_to_clear = None;
        self.nif_animator.active_animations.retain(|name, anim| {
            let is_same_priority = anim.priorities == priorities;
            let is_same_group = name == group_name;

            if is_same_priority && !is_same_group {
                animation_to_clear = Some(name.clone());
                false // Evict from active_animations
            } else {
                true
            }
        });
        if animation_already_exists {
            if let Some(animation_to_clear) = animation_to_clear {
                self.clear_animation(&animation_to_clear);
            }
            return;
        }
        let Some(animation_definition) = self.nif_animator.animation_definitions.get(group_name)
        else {
            return;
        };
        let active_animation = ActiveAnimation {
            clip_handle: animation_definition.clip_handle.clone(),
            node_index: animation_definition.node_index,
            loop_count,
            blend_mask,
            next_clip_name: animation_definition.next_clip_name.clone(),
            priorities,
            auto_remove: auto_disable,
            speed_mult,
        };
        // Only play it if the clip is shorter than where we wanted to play it from
        let should_play = animations
            .get(&animation_definition.clip_handle)
            .map(|clip| start_time < clip.duration())
            .unwrap_or(false);
        let events_before_start_time: Vec<&NifEvent> = animation_definition
            .animation_events
            .iter()
            .filter_map(|(time, event)| {
                if *time < start_time {
                    Some(event)
                } else {
                    None
                }
            })
            .collect();
        for event in events_before_start_time {
            commands.trigger(*event);
        }
    }
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

        // Add the new animation to the active list
        self.nif_animator
            .active_animations
            .insert(group_key.to_string(), new_animation.clone());

        // Recalculate and apply masks for _all_ active animations
        self.recalculate_and_apply_masks();

        // Start playback
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
    /// 3.  The main `CharacterController` component (to prevent stale state bugs).
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

            return Some(removed_anim);
        }
        None
    }
    pub fn set_repeating(&mut self, anim_name: &str) {
        if let Some(active_animation) = self.nif_animator.active_animations.get(anim_name) {
            if let Some(anim) = self
                .animation_player
                .animation_mut(active_animation.node_index)
            {
                anim.repeat();
            }
        }
    }

    /// Clears movement, jumping, and combat animations
    pub fn clear_all_other_animations(&mut self) {
        self.clear_animation(&self.character_controller.movement.current_group.clone());
        self.clear_animation(&self.character_controller.jumping.current_group.clone());
        self.clear_animation(&self.character_controller.combat.current_group.clone());
    }

    /// Recalculates the effective blend mask for every active animation based on priority and applies it.
    pub fn recalculate_and_apply_masks(&mut self) {
        let mut effective_masks: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();

        // For each body region, find the animation with the highest priority that wants to affect it.
        for i in 0..NUM_DISCRETE_REGIONS {
            let region_mask = 1 << i;
            let mut winner_key: Option<String> = None;
            let mut highest_priority = Priority::Default;

            for (key, anim) in &self.nif_animator.active_animations {
                //  read the animation's original, intended blend mask.
                let intended_mask = anim.blend_mask;
                if intended_mask.bits() & region_mask != 0 {
                    if anim.priorities[i] >= highest_priority {
                        highest_priority = anim.priorities[i];
                        winner_key = Some(key.clone());
                    }
                }
            }

            // the highest priority gets the region
            if let Some(key) = winner_key {
                *effective_masks.entry(key).or_insert(0) |= region_mask;
            }
        }

        // Apply the newly calculated effective masks to the visual animation graph.
        if let Some(graph) = self.anim_graphs.get_mut(self.graph_handle) {
            for (key, _anim) in &mut self.nif_animator.active_animations {
                let effective_mask = effective_masks.get(key).cloned().unwrap_or(0);

                // Update visual mask in the animation graph
                if let Some(definition) = self.nif_animator.animation_definitions.get(key) {
                    if let Some(node) = graph.get_mut(definition.node_index) {
                        node.mask = !effective_mask; // unmask these parts to have them contribute
                    }
                }
            }
        }
    }
}
