use bevy::{
    animation::{
        AnimationPlayer,
        graph::{AnimationGraph, AnimationGraphHandle},
    },
    prelude::*,
};
use bevy_nif::nif::animation::{NifAnimator, SkeletonMap};
use cdda_bevy::{
    AnimationState, NetworkEvent, RequestedAnimation,
    client::{
        defs::{
            AnimationEvent, Direction, IdToEntityMap, LocalPlayer, NifAnimatorLink, PlayerState,
        },
        events::RequestAnimation,
    },
};

pub fn play_animation(
    nif_animator: &NifAnimator,
    anim_graphs: &mut ResMut<Assets<AnimationGraph>>,
    animation_player: &mut AnimationPlayer,
    graph_handle: &AnimationGraphHandle,
    requested_animation: RequestedAnimation,
    player_state: &mut PlayerState,
) {
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
    mut player_query: Query<(&NifAnimatorLink, &mut PlayerState)>,
    mut commands: Commands,
    id_to_entity_map: Res<IdToEntityMap>,
) {
    let Some(entity) = id_to_entity_map.0.get(&trigger.entity_id) else {
        return;
    };
    let Ok((nif_animator_link, mut player_state)) = player_query.get_mut(*entity) else {
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
    if trigger.networked {
        let animation = AnimationEvent::RequestedAnimationEvent {
            requested_animation: trigger.requested_animation.clone(),
            target_id: trigger.entity_id,
        };
        println!("triggered animation {}", trigger.networked);
        commands.trigger(NetworkEvent::Animation(animation));
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
