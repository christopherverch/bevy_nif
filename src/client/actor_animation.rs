use once_cell::sync::Lazy;
use std::collections::HashSet;

use bevy::{
    animation::{
        AnimationPlayer, RepeatAnimation,
        graph::{AnimationGraph, AnimationGraphHandle},
    },
    prelude::*,
};

use bevy_nif::nif::animation::{ActiveAnimNif, NifAnimator};
use cdda_bevy::{
    AnimationConfig, NetworkEvent, RepeatAnim,
    client::{
        animation::get_anim_components_from_skel_id,
        defs::{IdToEntityMap, NetworkAnimEvent, NifAnimatorLink, PlayerState},
        events::{RequestAnimation, SetAnimSpeedSeekEvent},
    },
};
pub fn play_animation(
    nif_animator: &mut NifAnimator,
    anim_graphs: &mut ResMut<Assets<AnimationGraph>>,
    animation_player: &mut AnimationPlayer,
    graph_handle: &AnimationGraphHandle,
    player_state: &mut PlayerState,
    skeleton_id: u64,
    new_anim_config: &AnimationConfig,
) {
    let new_animation_index = nif_animator
        .animation_definitions
        .get(&new_anim_config.name)
        .unwrap()
        .node_index;
    let mut animation_was_already_playing = false;
    // loop through all animations, retaining only those that aren't finished

    nif_animator
        .active_animations
        .retain(|_name, active_animation| {
            let animation_in_player =
                match animation_player.animation(active_animation.animation_node_index) {
                    Some(anim) => anim, // If it exists, use it.
                    None => {
                        // If it's not in the animation_player (e.g., node index is stale),
                        // then this active_animation entry is invalid. Remove it.
                        return false;
                    }
                };

            // Check if the animation (from the player) is finished.
            if animation_in_player.is_finished() {
                return false; // Remove if finished.
            }

            if active_animation.animation_node_index == new_animation_index {
                animation_was_already_playing = true;
            }
            // past this point we know all animations are valid

            // handle conflicting masks
            if active_animation.mask.intersects(new_anim_config.mask) {
                if new_anim_config.priority > active_animation.priority {
                    // this animation's priority is higher, unmask it and mask the current animation
                    // for the overlapping portions
                    if let Some(anim_graph) = anim_graphs.get_mut(graph_handle) {
                        if let Some(animation_graph_node) = anim_graph.get_mut(new_animation_index)
                        {
                            dbg!("setting anim mask");
                            animation_graph_node.mask = u64::MAX;
                        }
                    }
                }
            }
            true
        });
    if animation_was_already_playing {
        animation_player.stop(new_animation_index);
    }
    let repeat_behavior = match new_anim_config.repeat {
        RepeatAnim::Forever => RepeatAnimation::Forever,
        RepeatAnim::Count(count) => RepeatAnimation::Count(count),
        RepeatAnim::Never => RepeatAnimation::Never,
    };

    animation_player
        .play(new_animation_index)
        .repeat()
        .set_repeat(repeat_behavior);
    nif_animator.active_animations.insert(
        new_anim_config.name.clone(),
        ActiveAnimNif {
            animation_node_index: new_animation_index,
            name: new_anim_config.name.clone(),
            priority: new_anim_config.priority,
            mask: new_anim_config.mask,
        },
    );
    info!("playing animation {}", new_anim_config.name);
}
/*
pub fn play_animation_old(
    nif_animator: &mut NifAnimator,
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
            animation_player.play(*animation).seek_to(0.0);
        }
        RequestedAnimation::Common(AnimationState::Idle) => {
            animation = nif_animator.animations.get("idle").unwrap();
            animation_player.stop_all();
            nif_animator.active_animations.clear();
            animation_player.play(*animation).seek_to(0.0).repeat();
        }
        RequestedAnimation::LoopMovement { animation } => {
            let loop_animation = nif_animator.animations.get(&animation).unwrap();
            nif_animator.active_animations.push(ActiveAnimation {
                animation_node_index: *loop_animation,
                group_name: "loop animation".to_string(),
                priority: 0,
                mask: BlendMask::all(),
                target_weight: 1.0,
                transition_state: AnimationTransitionState::Sustained,
                auto_remove_when_finished: false,
                started: false,
            });
            animation_player
                .play(*loop_animation)
                .repeat()
                .set_repeat(RepeatAnimation::Forever);
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
*/

pub fn handle_animation_request(
    trigger: Trigger<RequestAnimation>,
    mut animator_q: Query<(
        &mut AnimationPlayer,
        &mut NifAnimator,
        &AnimationGraphHandle,
    )>,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    id_to_entity_map: Res<IdToEntityMap>,
    mut player_query: Query<(&NifAnimatorLink, &mut PlayerState)>,
    mut commands: Commands,
) {
    let Some(entity_to_animate) =
        get_anim_components_from_skel_id(trigger.skeleton_id, &id_to_entity_map)
    else {
        return;
    };

    let Ok((nif_animator_link, mut player_state)) = player_query.get_mut(*entity_to_animate) else {
        return;
    };
    let Ok((mut animation_player, mut nif_animator, graph_handle)) =
        animator_q.get_mut(nif_animator_link.0)
    else {
        return;
    };
    /*for (name, animation) in &nif_animator.animations {
        println!("animation: {}", name);
    }*/
    // if it's a run/walk/idle animation, stop every run/walk animation and play the new one
    if trigger.animation_config.name.contains("run")
        || trigger.animation_config.name.contains("idle")
        || trigger.animation_config.name.contains("jump")
    {
        stop_all_movement_animations(&mut nif_animator, &mut animation_player);
    }
    info!("trying to play anim {}", trigger.animation_config.name);
    play_animation(
        &mut nif_animator,
        &mut anim_graphs,
        &mut animation_player,
        graph_handle,
        &mut player_state,
        trigger.skeleton_id,
        &trigger.animation_config,
    );

    send_if_networked(
        trigger.networked,
        &trigger.animation_config,
        trigger.skeleton_id,
        &mut commands,
    );
}
pub fn handle_set_anim_speed_seek(
    trigger: Trigger<SetAnimSpeedSeekEvent>,
    mut animator_q: Query<(&mut AnimationPlayer, &mut NifAnimator)>,
    id_to_entity_map: Res<IdToEntityMap>,
    mut player_query: Query<&NifAnimatorLink>,
    mut commands: Commands,
) {
    let Some(entity_to_animate) =
        get_anim_components_from_skel_id(trigger.skeleton_id, &id_to_entity_map)
    else {
        return;
    };
    let Ok(nif_animator_link) = player_query.get_mut(*entity_to_animate) else {
        return;
    };
    let Ok((mut animation_player, nif_animator)) = animator_q.get_mut(nif_animator_link.0) else {
        return;
    };
    if trigger.networked {
        info!(
            "triggered networked setanimspeed {:?}: {}",
            trigger.animation, trigger.speed
        );
        commands.trigger(NetworkEvent::SetAnimSpeedSeek(trigger.clone()));
    }
    if let Some(animation) = nif_animator.animation_definitions.get(&trigger.animation) {
        if let Some(active_animation) = animation_player.animation_mut(animation.node_index) {
            active_animation.set_speed(trigger.speed);
            if let Some(time) = trigger.seek_to {
                active_animation.seek_to(time);
            }
        }
    }
}

fn send_if_networked(
    is_networked: bool,
    animation_config: &AnimationConfig,
    skeleton_id: u64,
    commands: &mut Commands,
) {
    if is_networked {
        let animation = NetworkAnimEvent::BevyAnimEvent {
            animation_config: animation_config.clone(),
            skeleton_id,
        };
        info!("triggered networked animation {:?}", animation_config);
        commands.trigger(NetworkEvent::Animation(animation));
    }
}
static MOVEMENT_ANIMATION_NAMES_SET: Lazy<HashSet<String>> = Lazy::new(|| {
    let base_names = [
        "runforward",
        "runleft",
        "runright",
        "runback",
        "runforward_loop",
        "runback_loop",
        "runleft_loop",
        "runright_loop",
        "walkforward",
        "walkback",
        "walkleft",
        "walkright",
        "walkforward_loop",
        "walkback_loop",
        "walkleft_loop",
        "walkright_loop",
        "jump",
        "idle",
    ];

    let mut final_set = HashSet::new();

    for &name_ref in base_names.iter() {
        // Add the original name
        final_set.insert(name_ref.to_string());

        // Create and add the "hh" suffixed version
        let modified_name = if let Some(base_name) = name_ref.strip_suffix("_loop") {
            format!("{}hh_loop", base_name)
        } else {
            format!("{}hh", name_ref)
        };
        final_set.insert(modified_name);
    }

    final_set
});
fn stop_all_movement_animations(
    nif_animator: &mut NifAnimator,
    animation_player: &mut AnimationPlayer,
) {
    for (name, index2) in &nif_animator.animation_definitions {
        println!("animation: {:?}", index2);
    }

    nif_animator
        .active_animations
        .retain(|animation_name, active_animation| {
            dbg!(animation_name);
            if MOVEMENT_ANIMATION_NAMES_SET
                .get(animation_name.as_str())
                .is_some()
            {
                animation_player.stop(active_animation.animation_node_index);
                return false;
            }
            true
        })
}
/*
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
                if let Some(walk_back_animation) = nif_animator.animation_definitions.get("jump") {
                    if let Some(pickprobe_animation) =
                        nif_animator.animation_definitions.get("jump")
                    {
                        if let Some(anim_graph_node) =
                            anim_graph.get_mut(pickprobe_animation.node_index)
                        {
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
}*/
