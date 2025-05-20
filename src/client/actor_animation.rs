use std::collections::HashMap;

use bevy::{
    animation::{
        AnimationPlayer, RepeatAnimation,
        graph::{AnimationGraph, AnimationGraphHandle},
    },
    prelude::*,
};
use bevy_nif::nif::animation::{NifAnimator, NifEvent, NifEventType, SkeletonMap};
use cdda_bevy::{
    AnimationState, RequestedAnimation,
    client::{
        animation::trigger_request_animation,
        defs::{AnimationEvent, Direction, LocalPlayer, NifAnimatorLink, PlayerState},
        events::{RequestAnimation, SoundEvent},
    },
};

use crate::send_data::queue_network_animation;
pub fn test_nif_trigger(
    trigger: Trigger<NifEvent>,
    mut commands: Commands,
    local_player_q: Query<&LocalPlayer>,
    mut animator_q: Query<(&mut AnimationPlayer, &NifAnimator, &AnimationGraphHandle)>,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    mut player_query: Query<(Entity, &NifAnimatorLink, &mut PlayerState)>,
) {
    let Some((player_entity, nif_animator_link, mut player_state)) = player_query
        .iter_mut()
        .find(|(_, _, player_state)| player_state.id == trigger.skeleton_id)
    else {
        return;
    };
    let Ok((mut animation_player, nif_animator, graph_handle)) =
        animator_q.get_mut(nif_animator_link.0)
    else {
        return;
    };
    println!(
        "nif event triggered: {:?} {}",
        trigger.event_type, nif_animator.skeleton_id
    );
    if local_player_q.get(player_entity).is_ok() {
        queue_network_animation(
            &mut player_state,
            AnimationEvent::NifAnimationEvent {
                event_type: trigger.event_type.clone(),
            },
        );
    }
    match &trigger.event_type {
        NifEventType::PlaySound { sound_name } => {
            if sound_name.contains("Left") {
                commands.trigger(SoundEvent("sounds/walkl_br.wav".to_string()));
            } else if sound_name.contains("Right") {
                commands.trigger(SoundEvent("sounds/walkr_br.wav".to_string()));
            } else {
            }
        }
        NifEventType::LoopAnimation { animation_name } => {
            play_animation(
                nif_animator,
                &mut anim_graphs,
                &mut animation_player,
                graph_handle,
                RequestedAnimation::LoopMovement {
                    animation: format!("{}_loop", animation_name),
                },
                &mut player_state,
            );
        }
        NifEventType::FreezeAnimation { animation_name } => {
            player_state.current_animation = AnimationState::Undefined;
            commands.trigger(RequestAnimation {
                entity: player_entity,
                requested_animation: RequestedAnimation::FreezeAnimation,
            });
            if animation_name == "" {}
        }
        NifEventType::ResumeIdle { animation_name } => {
            trigger_request_animation(
                commands.reborrow(),
                player_entity,
                AnimationState::Idle.into(),
                &mut player_state,
            );
            play_animation(
                nif_animator,
                &mut anim_graphs,
                &mut animation_player,
                graph_handle,
                AnimationState::Idle.into(),
                &mut player_state,
            );
            if animation_name == "" {}
        } //_ => {}
    }
}

pub fn play_animation(
    nif_animator: &NifAnimator,
    anim_graphs: &mut ResMut<Assets<AnimationGraph>>,
    animation_player: &mut AnimationPlayer,
    graph_handle: &AnimationGraphHandle,
    requested_animation: RequestedAnimation,
    player_state: &mut PlayerState,
) {
    //if this is the local player
    let _anim_graph = anim_graphs.get_mut(graph_handle).unwrap();
    let animation;
    /*for animation in &nif_animator.animations {
        println!("animation: {}", animation.0);
    }*/
    match requested_animation {
        RequestedAnimation::Common(AnimationState::Running { direction }) => {
            let animation = match direction {
                Direction::Forward => nif_animator.animations.get("runforward").unwrap(),
                Direction::Left => nif_animator.animations.get("runleft").unwrap(),
                Direction::Right => nif_animator.animations.get("runright").unwrap(),
                Direction::Backward => nif_animator.animations.get("runback").unwrap(),
            };
            animation_player.stop_all();
            animation_player.play(*animation).seek_to(0.0).repeat();
        }
        RequestedAnimation::Common(AnimationState::Idle) => {
            animation = nif_animator.animations.get("idle").unwrap();
            animation_player.stop_all();
            animation_player.play(*animation).seek_to(0.0).repeat();
        }
        RequestedAnimation::LoopMovement { animation } => {
            let loop_animation = nif_animator.animations.get(&animation).unwrap();
            animation_player.stop_all();
            animation_player
                .play(*loop_animation)
                .repeat()
                .set_repeat(bevy::animation::RepeatAnimation::Forever);
        }
        RequestedAnimation::Common(AnimationState::Jumping) => {
            let jump_animation = nif_animator.animations.get("jump").unwrap();
            animation_player.stop_all();
            animation_player
                .play(*jump_animation)
                .set_repeat(bevy::animation::RepeatAnimation::Never);
        }
        RequestedAnimation::FreezeAnimation => {
            let jump_animation = nif_animator.animations.get("jump").unwrap();
            if let Some(current_animation) = animation_player.animation_mut(*jump_animation) {
                current_animation.set_speed(0.0);
            }
        }
        RequestedAnimation::ResumeAnimation => {
            let jump_animation = nif_animator.animations.get("jump").unwrap();
            if let Some(current_animation) = animation_player.animation_mut(*jump_animation) {
                current_animation.set_speed(1.0);
            }
        }
        RequestedAnimation::Common(AnimationState::Knockdown) => {
            let jump_animation = nif_animator.animations.get("knockdown").unwrap();
            animation_player.stop_all();
            animation_player
                .play(*jump_animation)
                .set_repeat(bevy::animation::RepeatAnimation::Never);
            player_state.current_animation = AnimationState::Knockdown;
        }
        RequestedAnimation::Common(AnimationState::Death) => {
            let jump_animation = nif_animator.animations.get("death4").unwrap();
            animation_player.stop_all();
            animation_player
                .play(*jump_animation)
                .set_repeat(bevy::animation::RepeatAnimation::Never);
            player_state.current_animation = AnimationState::Knockdown;
        }
        _ => {}
    }
}
pub fn handle_animation_request(
    trigger: Trigger<RequestAnimation>,
    mut animator_q: Query<(&mut AnimationPlayer, &NifAnimator, &AnimationGraphHandle)>,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    skeleton_map_res: Res<SkeletonMap>,
    mut player_query: Query<(Entity, &NifAnimatorLink, &mut PlayerState)>,
    local_player_q: Query<&LocalPlayer>,
) {
    let Ok((player_entity, nif_animator_link, mut player_state)) =
        player_query.get_mut(trigger.entity)
    else {
        return;
    };
    let Some(_skeleton) = skeleton_map_res.skeletons.get(&player_state.id) else {
        return;
    };
    let Ok((mut animation_player, nif_animator, graph_handle)) =
        animator_q.get_mut(nif_animator_link.0)
    else {
        return;
    };
    //if this is the local player
    if local_player_q.get(player_entity).is_ok() {
        queue_network_animation(
            &mut player_state,
            AnimationEvent::RequestedAnimationEvent {
                requested_animation: trigger.requested_animation.clone(),
            },
        );
    }
    play_animation(
        nif_animator,
        &mut anim_graphs,
        &mut animation_player,
        graph_handle,
        trigger.requested_animation.clone(),
        &mut player_state,
    );
}
fn play_animation_helper(
    animation_player: &mut AnimationPlayer,
    animation_map: &HashMap<String, AnimationNodeIndex>,
    animation_name: &str,
    repeat_mode: RepeatAnimation,
    seek_to_start: bool,
) {
    match animation_map.get(animation_name) {
        Some(animation_handle) => {
            animation_player.stop_all(); // Stop all previous animations
            let playing_animation = animation_player.play(*animation_handle);
            playing_animation.set_repeat(repeat_mode);

            if seek_to_start {
                playing_animation.seek_to(0.0);
            }
        }
        None => {
            error!("Animation not found: {}", animation_name); // Use Bevy's logger
        }
    }
}
pub fn test_animations(
    mut animator_q: Query<(&mut AnimationPlayer, &NifAnimator, &AnimationGraphHandle)>,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    mut run_once: Local<bool>,
    skeleton_map_res: Res<SkeletonMap>,
) {
    if !*run_once {
        if animator_q.iter().len() < 2 {
            return;
        }
        for (id, skeleton) in &skeleton_map_res.skeletons {
            for (mut animation_player, nif_animator, graph_handle) in animator_q.iter_mut() {
                if nif_animator.skeleton_id != *id {
                    continue;
                }
                let anim_graph = anim_graphs.get_mut(graph_handle).unwrap();
                if let Some(walk_back_animation) = nif_animator.animations.get("jump") {
                    if let Some(pickprobe_animation) = nif_animator.animations.get("jump") {
                        if let Some(anim_graph_node) = anim_graph.get_mut(*pickprobe_animation) {
                            let mut mask: u64 = 0;
                            for bone_data in skeleton.get_all_children("Bip01 L Thigh") {
                                mask |= 1 << bone_data.id.0 as u32;
                            }
                            for bone_data in skeleton.get_all_children("Bip01 R Thigh") {
                                mask |= 1 << bone_data.id.0 as u32;
                            }
                            println!("mask: {:b}", mask);
                            anim_graph_node.mask = mask as u64;
                        }
                        let mask = 0;

                        /*
                        for bone_data in skeleton.get_all_children("Bip01 R Clavicle") {
                            mask |= 1 << bone_data.id.0 as u32;
                        }
                        for bone_data in skeleton.get_all_children("Bip01 L Clavicle") {
                            mask |= 1 << bone_data.id.0 as u32;
                        }
                        */
                        let walk_anim_graph_node =
                            anim_graph.get_mut(*walk_back_animation).unwrap();
                        walk_anim_graph_node.mask = mask;
                        animation_player
                            .play(*pickprobe_animation)
                            .seek_to(0.0)
                            .repeat();
                        animation_player.play(*walk_back_animation).repeat();
                        *run_once = true;
                    }
                }
            }
        }
    }
}
