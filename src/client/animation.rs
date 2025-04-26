use crate::AnimationState;
use crate::client::defs::{MaskGroup, PlayerState};
use crate::client::events::RequestAnimation;
use crate::client::modular_characters::AnimationEntityLink;
use bevy::prelude::*;

use super::defs::Animator;
pub fn handle_animation_request(
    trigger: Trigger<RequestAnimation>,
    mut animation_query: Query<(Entity, &mut AnimationPlayer)>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut player_query: Query<(&mut PlayerState, &AnimationEntityLink, &Animator)>,
    mut player_state_local: ResMut<PlayerState>,
) {
    let mut use_local_state = false;
    if player_state_local.id == trigger.id {
        use_local_state = true;
    }
    for (mut player_state, animation_entity_link, animator) in player_query.iter_mut() {
        //check if this is the local player, if so use the PlayerState resource
        //since the component won't be in sync

        if player_state.id != trigger.id {
            continue;
        }
        let state_to_use: &mut PlayerState;

        if use_local_state {
            state_to_use = &mut player_state_local;
        } else {
            state_to_use = &mut player_state;
        }
        if trigger.animation_state == AnimationState::Swinging {
            state_to_use.secondary_animation = trigger.animation_state;
        } else {
            state_to_use.current_animation = trigger.animation_state;
        }
        println!(
            "animation triggered with id {} and animationstate {:?}, animationlink {:?}, state id: {}, is attacking: {}",
            trigger.id,
            trigger.animation_state,
            animation_entity_link,
            state_to_use.id,
            state_to_use.is_attacking,
        );
        println!("{:?} asdfasdf {:?}", animator.animations, animator.graph);

        play_animation_local(
            &mut animation_query,
            animation_entity_link,
            animator,
            &mut graphs,
            &state_to_use,
            trigger.animation_state,
        );
        break;
    }
}

pub fn play_animation_local(
    animation_query: &mut Query<(Entity, &mut AnimationPlayer)>,
    animation_entity_link: &AnimationEntityLink,
    animations: &Animator,
    graphs: &mut Assets<AnimationGraph>,
    player_state: &PlayerState,
    animation_state: AnimationState,
) {
    if let Ok((_, mut animation_player)) = animation_query.get_mut(animation_entity_link.0) {
        //TODO:: add a transition between animations
        let graph_handle = &animations.graph;

        let animation_state = animation_state;
        let animation_state_index = animation_state as usize;
        // Play the main animation (e.g., Run, Idle)
        if let Some(graph) = graphs.get_mut(graph_handle) {
            let swing_index = animations.animations[AnimationState::Swinging as usize];

            for &node_index in animations.animations.iter() {
                if let Some(node) = graph.get_mut(node_index) {
                    update_mask(node, node_index, swing_index, player_state);

                    if let Some(clip) = animation_player.animation_mut(node_index) {
                        println!("clip weight: {}", clip.weight());
                        let is_current = node_index == animations.animations[animation_state_index];
                        let is_swinging_index = node_index == swing_index;
                    }
                }
            }
        }

        if animation_state == AnimationState::Swinging {
            if player_state.is_ready_weapon && !player_state.is_attacking {
                println!("playing ready animation (paused swing)");
                let playback = animation_player
                    .play(animations.animations[animation_state_index])
                    .seek_to(0.0); // Start at the beginning
            } else if !player_state.is_ready_weapon {
            }
        } else {
        }
    }
}
fn update_mask(
    node: &mut AnimationGraphNode,
    node_index: AnimationNodeIndex,
    swinging_index: AnimationNodeIndex,
    player_state: &PlayerState,
) {
    println!("node mask for {:?}: {}", node_index, node.mask);
    if node_index != swinging_index {
        println!("unblocking arms");
        node.mask = 0;
        if player_state.is_attacking {
            println!("blocking arms");
            node.mask |= 1 << MaskGroup::ARMS as u64;
            node.mask &= !(1 << MaskGroup::TORSO as u64);
        }
    }
}
fn determine_weight(
    is_desired_animation_clip: bool,
    animation_state: AnimationState,
    animations: &Animator,
    player_state: &PlayerState,
) -> f32 {
    // Default weight for all animations: 0.0
    let mut weight = 0.0;
    if is_desired_animation_clip {
        weight = 1.0;
    }
    // If the animation is swinging, handle specific conditions
    if is_swinging_index {
        // If the player is not ready and the animation is swinging, set weight to 0
        if !player_state.is_ready_weapon && !player_state.is_attacking {
            weight = 0.0; // No swinging animation component
        } else {
            weight = 1.0; // Active swinging animation
        }
    }

    if animation_state == AnimationState::Swinging
        && node_index == animations.animations[player_state.current_animation as usize]
    {
        weight = 1.0;
    }
    weight
} // Set the final weight for the clip
