use bevy::{animation::RepeatAnimation, prelude::*};
use bevy_nif::nif_animation::{NifAnimator, bevy_types::ActiveAnimation};

use crate::{
    animation::{
        AnimationContext, NifAnimatorLink, animation_helpers::get_anim_components_for_creature,
    },
    defs::IdToEntityMap,
    mechanics::creature_stats::CreatureStats,
};
pub fn handle_animation_transitions(
    anim_clips: Res<Assets<AnimationClip>>,
    actor_query: Query<(&CreatureStats, &NifAnimatorLink)>,
    mut animator_q: Query<(
        &mut AnimationPlayer,
        &mut NifAnimator,
        &AnimationGraphHandle,
    )>,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    id_to_entity_map: Res<IdToEntityMap>,
) {
    for (creature_stats, creature_animator_link) in actor_query.iter() {
        let Some((_, mut animation_player, mut animator, graph_handle)) =
            get_anim_components_for_creature(
                creature_stats.network_id,
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
        let mut transitions_to_perform = Vec::new();
        let mut animations_to_remove = Vec::new();

        // Step 1: Identify finished animations.
        for (group_name, active_anim) in animation_context.nif_animator.active_animations.iter() {
            // Looping animations continue forever
            let Some(animation) = animation_context
                .animation_player
                .animation(active_anim.node_index)
            else {
                continue;
            };
            if animation.repeat_mode() == RepeatAnimation::Forever {
                continue;
            }
            if let Some(anim_state) = animation_context
                .animation_player
                .animation(active_anim.node_index)
            {
                if NifAnimator::is_finished(
                    active_anim.clip_handle.clone(),
                    &anim_clips,
                    anim_state,
                ) {
                    if let Some(next_clip_name) = &active_anim.next_clip_name {
                        transitions_to_perform.push((
                            group_name.clone(),
                            next_clip_name.clone(),
                            active_anim.next_should_loop,
                        ));
                    } else if animation.repeat_mode() == RepeatAnimation::Never {
                        animations_to_remove.push(group_name.clone());
                    }
                }
            }
        }

        // Step 2: Execute state changes for finished animations.
        for name in animations_to_remove {
            animation_context.clear_animation(&name);
        }
        // Step 3: Handle intro-to-loop transitions.
        for (current_anim_name, next_anim_name, should_loop) in transitions_to_perform {
            if let Some(transitioning_anim) = animation_context.clear_animation(&current_anim_name)
            {
                if let Some(next_anim_definition) = animation_context
                    .nif_animator
                    .animation_definitions
                    .get(&next_anim_name)
                {
                    let new_anim = ActiveAnimation {
                        node_index: next_anim_definition.node_index,
                        clip_handle: next_anim_definition.clip_handle.clone(),
                        anim_type: next_anim_definition.anim_type,
                        // Use the old animation's blend mask
                        blend_mask: transitioning_anim.blend_mask,
                        next_clip_name: next_anim_definition.next_clip_name.clone(),
                        next_should_loop: false,
                        priorities: transitioning_anim.priorities,
                        auto_remove: transitioning_anim.auto_remove,
                        speed_mult: transitioning_anim.speed_mult,
                    };

                    animation_context.play_animation_transition(
                        &next_anim_name,
                        new_anim,
                        None,
                        if should_loop { u32::MAX } else { 0 },
                    );
                }
            }
        }
    }
}
