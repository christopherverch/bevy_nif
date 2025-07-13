use bevy::animation::{AnimationPlayer, graph::AnimationGraphHandle};
use bevy_ecs::{entity::Entity, system::Query, world::Mut};
use bevy_nif::nif_animation::NifAnimator;

use crate::client::defs::{CreatureStats, IdToEntityMap};

pub fn get_anim_components_for_creature<'a>(
    creature_stats: &'a CreatureStats,
    id_to_entity_map: &'a IdToEntityMap,
    animator_q: &'a mut Query<(
        &mut AnimationPlayer,
        &mut NifAnimator,
        &AnimationGraphHandle,
    )>,
) -> Option<(
    &'a Entity,
    Mut<'a, AnimationPlayer>,
    Mut<'a, NifAnimator>,
    &'a AnimationGraphHandle,
)> {
    let skeleton_id = creature_stats.entity_id;
    let entity_to_animate = id_to_entity_map.0.get(&skeleton_id)?;
    let nif_animator_link = creature_stats.nif_animator_link?;
    let Ok((animation_player, nif_animator, graph_handle)) = animator_q.get_mut(nif_animator_link)
    else {
        return None;
    };
    Some((
        entity_to_animate,
        animation_player,
        nif_animator,
        graph_handle,
    ))
}

/*
fn play_animation_primary(
    animator: &Animator,
    index: AnimationNodeIndex,
    animation_player: &mut AnimationPlayer,
    requested_animation: AnimationState,
    player_state: &PlayerState,
) {
    let requested_animation_index = AnimationState::from(requested_animation) as usize;
    if let Some(clip) = animation_player.animation_mut(index) {
        let is_requested_animation_clip = animator.animations[requested_animation_index] == index;
        clip.set_weight(determine_weight(
            is_requested_animation_clip,
            requested_animation,
            animator,
            index,
            player_state,
        ));
    }
    animation_player
        .play(animator.animations[requested_animation as usize])
        .seek_to(0.0); // Start from the beginning of the animation
}
pub fn play_animation_secondary(
    animator: &Animator,
    animation_graph_node: &mut AnimationGraphNode,
    index: AnimationNodeIndex,
    animation_player: &mut AnimationPlayer,
    requested_animation: RequestedAnimation,
) {
    let swing_index = animator.animations[AnimationState::Swinging as usize];
    let requested_animation_index = AnimationState::from(requested_animation) as usize;
    update_mask(
        animation_graph_node,
        index,
        swing_index,
        requested_animation,
    );

    if let Some(clip) = animation_player.animation_mut(index) {
        if animator.animations[requested_animation_index] == index {
            if requested_animation == RequestedAnimation::SwingInactive {
                clip.set_weight(0.0);
            } else {
                clip.set_weight(1.0);
            }
        }
    }

    run_swinging_playback(
        animation_player,
        animator,
        requested_animation_index,
        requested_animation,
    );
}
fn update_mask(
    animation_graph_node: &mut AnimationGraphNode,
    index: AnimationNodeIndex,
    swinging_index: AnimationNodeIndex,
    requested_animation: RequestedAnimation,
) {
    //don't modify the swinging animation mask, it always blocks the body and not the arms
    if index != swinging_index {
        if requested_animation == RequestedAnimation::SwingInactive {
            //unblock the arms
            animation_graph_node.mask = 0;
        } else {
            animation_graph_node.mask |= 1 << MaskGroup::ARMS as u64;
            animation_graph_node.mask &= !(1 << MaskGroup::TORSO as u64);
        }
    }
}
fn determine_weight(
    is_requested_animation_clip: bool,
    requested_animation: AnimationState,
    animations: &Animator,
    index: AnimationNodeIndex,
    player_state: &PlayerState,
) -> f32 {
    // Default weight for all animations: 0.0
    let mut weight = 0.0;
    if is_requested_animation_clip {
        weight = 1.0;
    }
    // If the animation is swinging, handle specific conditions
    let swing_anim = animations.animations[AnimationState::Swinging as usize];
    if index == swing_anim {
        // If the player weapon is not readied and the animation is swinging, set weight to 0
        if player_state.attack_state == AttackState::Inactive {
            weight = 0.0; // No swinging animation mixed with current animation
        } else {
            weight = 1.0; // Active swinging animation
        }
    }
    //if swinging animation was requested, make sure we don't set the weight of our main animation
    //to 0 (running, idle, etc)
    if requested_animation == AnimationState::Swinging
        && index == animations.animations[player_state.current_animation as usize]
    {
        weight = 1.0;
    }
    weight
}
pub fn set_third_person_animation_weights(
    trigger: Trigger<ToggledThirdPerson>,
    mut animation_query: Query<(Entity, &mut AnimationPlayer)>,
    mut player_query: Query<(&MeshEntityLink, &mut PlayerState)>,
    mut mesh_query: Query<(&AnimationEntityLink, &Animator)>,
    mut commands: Commands,
) {
    println!("toggled third person");
    let Some((player_state, animation_entity_link, animator)) =
        find_player_and_mesh(trigger.entity, &mut player_query, &mut mesh_query)
    else {
        println!("failed to find mesh!");
        return;
    };
    let Some(animation_entity) = animation_entity_link.0.get("models/main_skeleton.glb") else {
        return;
    };
    if let Ok((_, mut animation_player)) = animation_query.get_mut(*animation_entity) {
        for &index in animator.animations.iter() {
            if let Some(clip) = animation_player.animation_mut(index) {
                if index == animator.animations[player_state.current_animation as usize] {
                    if !trigger.is_third_person {
                        clip.set_weight(0.0);
                    }
                }
            }
        }
        let requested = if trigger.is_third_person {
            player_state.current_animation
        } else {
            AnimationState::FirstPersonMoving
        };

        commands.trigger(RequestAnimation {
            entity: trigger.entity,
            requested_animation: requested.into(),
        });
    }
}
fn find_player_and_mesh<'a>(
    entity: Entity,
    player_query: &'a mut Query<(&MeshEntityLink, &mut PlayerState)>,
    mesh_query: &'a mut Query<(&AnimationEntityLink, &Animator)>,
) -> Option<(Mut<'a, PlayerState>, &'a AnimationEntityLink, &'a Animator)> {
    let (mesh_link, player_state) = player_query.get_mut(entity).ok()?;
    let mesh_entity = mesh_link.0?;
    let (animation_entity_link, animator) = mesh_query.get(mesh_entity).ok()?;
    Some((player_state, animation_entity_link, animator))
}
fn run_swinging_playback(
    animation_player: &mut AnimationPlayer,
    animator: &Animator,
    requested_animation_index: usize,
    requested_animation: RequestedAnimation,
) {
    if let Some(playing_animation) =
        animation_player.animation_mut(animator.animations[requested_animation_index])
    {
        if playing_animation.is_finished() {
            println!("stopping animation");
            //stop the finished animation since it won't play unless we stop then start it
            animation_player.stop(animator.animations[requested_animation_index]);
        }
    }
    let playback = animation_player.play(animator.animations[requested_animation_index]);
    match requested_animation {
        RequestedAnimation::SwingReady => {
            playback.seek_to(0.0).pause();
        }
        RequestedAnimation::SwingCharging => {
            playback.seek_to(0.0).set_speed(1.0).resume();
        }
        RequestedAnimation::SwingReleasing => {
            //if we charged so much we ended the animation
            if playback.seek_time() == 0.0 {
                playback.set_seek_time(0.8);
            }
            playback.set_speed(-2.5).resume();
        }
        _ => {}
    }
}
*/
