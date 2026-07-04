use std::borrow::Cow;

use bevy::animation::AnimationPlayer;
use bevy::prelude::*;
use bevy_nif::nif_animation::bevy_types::{ActiveAnimation, AnimType, ManualNifEvent, Priority};
use bevy_nif::nif_animation::{BlendMask, NUM_DISCRETE_REGIONS, NifAnimator};
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
    /// Plays a new animation and recalculates all priority-based masks.
    pub fn play_animation<'a>(
        &mut self,
        animations: &Res<Assets<AnimationClip>>,
        group_name: impl Into<Cow<'a, str>>,
        extra_suffix: &str,
        priorities: [Priority; 4],
        blend_mask: BlendMask,
        auto_disable: bool,
        start_at: AnimType,
        speed_mult: f32,
        start_time: f32,
        loop_count: u32,
        commands: &mut Commands,
        //TODO:: add loop_fallback, which determines whether we loop the full intro->loop->outro sequence,
        //or just loop the _loop clip
    ) {
        // Early return for invalid input
        let group_name = group_name.into();
        if group_name.is_empty() {
            return;
        }
        let full_name = if !extra_suffix.is_empty() {
            Cow::Owned(format!("{}{}", group_name, extra_suffix))
        } else {
            group_name
        };

        let mut animation_already_exists = false;
        // Already playing, just update the priority
        if let Some(anim) = self
            .nif_animator
            .active_animations
            .get_mut(full_name.as_ref())
        {
            anim.priorities = priorities;
            animation_already_exists = true;
        }
        let mut animation_to_clear = None;
        self.nif_animator.active_animations.retain(|name, anim| {
            let is_same_priority = anim.priorities == priorities;
            let is_same_group = *name == full_name;

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
        let full_name_with_intro_outro = match start_at {
            AnimType::Intro => full_name.clone(),
            AnimType::Loop => Cow::Owned(format!("{}{}", full_name, "_loop")),
            AnimType::Outro => Cow::Owned(format!("{}{}", full_name, "_outro")),
            _ => full_name.clone(),
        };

        let Some(animation_definition) = self
            .nif_animator
            .animation_definitions
            .get(full_name_with_intro_outro.as_ref())
        else {
            return;
        };
        let mut new_animation = ActiveAnimation {
            clip_handle: animation_definition.clip_handle.clone(),
            node_index: animation_definition.node_index,
            anim_type: animation_definition.anim_type,
            blend_mask,
            next_clip_name: animation_definition.next_clip_name.clone(),
            priorities,
            auto_remove: auto_disable,
            speed_mult,
            next_should_loop: false,
        };
        let Some(animation_clip) = animations.get(&animation_definition.clip_handle) else {
            return;
        };
        // Only play it if the clip is shorter than where we wanted to play it from
        let should_play = start_time < animation_clip.duration();
        if !should_play {
            return;
        }
        // TODO:: seek_to does this for us automatically
        let events_before_start_time: Vec<&ManualNifEvent> = animation_definition
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
            commands.trigger(event.clone());
        }

        // Recalculate and apply masks for all active animations

        let node_index = new_animation.node_index;
        let speed_multiplier = new_animation.speed_mult;

        // Start playback
        let anim_state = self
            .animation_player
            .play(node_index)
            .set_speed(speed_multiplier);
        anim_state.seek_to(start_time);

        if loop_count == u32::MAX {
            // Assume the caller actually means they want to repeat the "loop" animation, rather
            // than the intro
            if animation_definition.anim_type == AnimType::Loop {
                anim_state.repeat();
            } else {
                // Mark the next animation (the loop) as looping instead
                new_animation.next_should_loop = true;
            }
        } else {
            anim_state.set_repeat(bevy::animation::RepeatAnimation::Never);
        }
        println!(
            "Playing anim '{}' with speed {:.2} (Looping: {:?})",
            full_name,
            speed_multiplier,
            anim_state.repeat_mode()
        );
        self.nif_animator
            .active_animations
            .insert(full_name.to_string(), new_animation);
        self.recalculate_and_apply_masks();
    }
    /// Plays a new animation and recalculates all priority-based masks.
    pub fn play_animation_transition(
        &mut self,
        group_key: &str,
        new_animation: ActiveAnimation,
        start_time: Option<f32>,
        loop_count: u32,
    ) {
        // Stop any animation that is an intro or outro to this one.
        let base_name = match new_animation.anim_type {
            AnimType::Intro => group_key,
            AnimType::Loop => group_key.strip_suffix("_loop").unwrap(),
            AnimType::Outro => group_key.strip_suffix("_outro").unwrap(),
            _ => {
                group_key
                //TODO:: may be some unexpected behavior with other clips if they have
                //loops etc, not sure if just returning group_key is fine here
            }
        };
        // Clear the animation we're transitioning from
        self.clear_animation(base_name);

        let node_index = new_animation.node_index;
        let speed_multiplier = new_animation.speed_mult;
        // Recalculate and apply masks for _all_ active animations

        // Start playback
        let anim_state = self
            .animation_player
            .play(node_index)
            .set_speed(speed_multiplier);
        if let Some(time) = start_time {
            anim_state.seek_to(time);
        }

        if loop_count == u32::MAX {
            anim_state.repeat();
        } else {
            anim_state.set_repeat(bevy::animation::RepeatAnimation::Never);
        }
        println!(
            "Playing anim '{}' with speed {:.2} (Looping: {})",
            group_key,
            speed_multiplier,
            loop_count == u32::MAX
        );

        // Add the new animation to the active list
        self.nif_animator
            .active_animations
            .insert(base_name.to_string(), new_animation);
        self.recalculate_and_apply_masks();
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
            dbg!("Stopping animation group: '{}'", group_name);

            // Stop playback in Bevy
            self.animation_player.stop(removed_anim.node_index);
            if let Some(mut graph) = self.anim_graphs.get_mut(self.graph_handle) {
                if let Some(node) = graph.get_mut(removed_anim.node_index) {
                    node.mask = !0; // Mask out (do not animate) all bones for this animation
                }
            }

            // After removing, recalculate and apply masks for all remaining animations.
            self.recalculate_and_apply_masks();

            return Some(removed_anim);
        }
        None
    }
    // Set either this animation as looping, or mark that its "next_clip_name" should be looping
    // This way if 1 shots were marked as looping, they loop, but if a clip like knockdown is
    // playing, we loop the knockdown_loop clip instead of the knockdown portion, which is what the
    // caller intends
    pub fn set_repeating(&mut self, anim_name: &str, repeating: bool) {
        if let Some(active_animation) = self.nif_animator.active_animations.get_mut(anim_name) {
            // If this is an intro, assume the caller meant they wanted the _loop animation to loop
            // instead of this one
            if active_animation.anim_type == AnimType::Intro {
                active_animation.next_should_loop = true;
                return;
            } else if active_animation.anim_type == AnimType::Outro {
                // We never want to repeat outro animations, the caller will re-run the animation
                // and tell it to repeat instead
                return;
            }
            if let Some(anim) = self
                .animation_player
                .animation_mut(active_animation.node_index)
            {
                if repeating {
                    anim.repeat();
                } else {
                    let seek_time = anim.seek_time();
                    // "replay" it to reset the loop completions, and seek back to where we were
                    // so it plays to the end of the final loop
                    anim.replay();
                    anim.set_seek_time(seek_time);
                    anim.set_repeat(bevy::animation::RepeatAnimation::Never);
                }
            }
        }
    }

    /// Recalculates the effective blend mask for every active animation based on priority and applies it.
    pub fn recalculate_and_apply_masks(&mut self) {
        // First, reset all nodes to "masked out" (or default state)
        for active_anim in &self.nif_animator.animation_definitions {
            if let Some(mut graph) = self.anim_graphs.get_mut(self.graph_handle) {
                if let Some(node) = graph.get_mut(active_anim.1.node_index) {
                    node.mask = !0;
                }
            }
        }

        let mut sorted_keys: Vec<_> = self.nif_animator.active_animations.keys().collect();
        sorted_keys.sort();
        // For each body region, find the animation with the highest priority that wants to affect it.
        for i in 0..NUM_DISCRETE_REGIONS {
            let region_mask = 1 << i;
            let mut winner_key: Option<String> = None;
            let mut highest_priority: Option<Priority> = None;

            for (key, anim) in &self.nif_animator.active_animations {
                //  read the animation's original, intended blend mask.
                if anim.blend_mask.bits() & region_mask != 0 {
                    if highest_priority.is_none() || anim.priorities[i] > highest_priority.unwrap()
                    {
                        highest_priority = Some(anim.priorities[i]);
                        winner_key = Some(key.clone());
                    }
                }
            }

            // the highest priority gets the region
            if let Some(key) = winner_key {
                let actual_key = match self.nif_animator.active_animations[&key].anim_type {
                    AnimType::Intro => key,
                    AnimType::Loop => format!("{}_loop", key),
                    AnimType::Outro => format!("{}_outro", key),
                    _ => {
                        //TODO::
                        key
                    }
                };

                dbg!(&actual_key);
                let def = &self.nif_animator.animation_definitions[&actual_key];
                if let Some(mut graph) = self.anim_graphs.get_mut(self.graph_handle) {
                    if let Some(node) = graph.get_mut(def.node_index) {
                        node.mask &= !region_mask; // unmask these parts to have them contribute
                    }
                }
            }
        }
    }
    pub fn adjust_anim_speed(&mut self, anim: &str, new_speed: f32) {
        if let Some(anim) = self.nif_animator.active_animations.get(anim) {
            if let Some(anim_playback) = self.animation_player.animation_mut(anim.node_index) {
                anim_playback.set_speed(new_speed);
            }
        }
    }
    pub fn is_playing(&self, anim: &str) -> bool {
        self.nif_animator.active_animations.contains_key(anim)
    }
    pub fn get_anim_info(&self, anim: &str) -> f32 {
        if let Some(active_anim) = self.nif_animator.active_animations.get(anim) {
            if let Some(anim) = self.animation_player.animation(active_anim.node_index) {
                return anim.seek_time();
            }
        }

        0.0
    }
    pub fn get_velocity(&self, anim: &str) -> f32 {
        let Some(anim_def) = self.nif_animator.animation_definitions.get(anim) else {
            return 0.0;
        };
        anim_def.base_velocity.length()
    }
}
