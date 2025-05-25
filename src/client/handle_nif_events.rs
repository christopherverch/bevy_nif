use bevy::{
    animation::{
        AnimationPlayer,
        graph::{AnimationGraph, AnimationGraphHandle},
    },
    prelude::*,
};
use bevy_nif::nif::animation::{NifAnimator, NifEvent, NifEventType};
use cdda_bevy::{
    AnimationState, NetworkEvent, RequestedAnimation,
    client::{
        animation::trigger_request_animation,
        defs::{AnimationEvent, IdToEntityMap, LocalPlayer, NifAnimatorLink, PlayerState},
        events::{RequestAnimation, SoundEvent},
    },
};

use crate::actor_animation::play_animation;

pub fn handle_nif_trigger(
    trigger: Trigger<NifEvent>,
    mut commands: Commands,
    local_player_q: Query<&LocalPlayer>,
    mut animator_q: Query<(&mut AnimationPlayer, &NifAnimator, &AnimationGraphHandle)>,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    mut player_query: Query<(Entity, &NifAnimatorLink, &mut PlayerState, &Transform)>,
    id_to_entity_map: Res<IdToEntityMap>,
) {
    let Some(entity) = id_to_entity_map.0.get(&trigger.skeleton_id) else {
        return;
    };
    let Ok((player_entity, nif_animator_link, mut player_state, player_transform)) =
        player_query.get_mut(*entity)
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
    let mut is_local_player = false;
    if local_player_q.get(player_entity).is_ok() {
        println!("is local player");
        is_local_player = true;
        let animation_event = AnimationEvent::NifAnimationEvent {
            event_type: trigger.event_type.clone(),
        };
        commands.trigger(NetworkEvent::Animation(animation_event));
    }
    match &trigger.event_type {
        NifEventType::PlaySound { sound_name } => {
            if sound_name.contains("Left") {
                commands.trigger(SoundEvent {
                    sound_anchor_entity: Some(player_entity),
                    translation: None,
                    sound_path: "sounds/walkl_br.wav".to_string(),
                    is_attached_to_local_player: is_local_player,
                });
            } else if sound_name.contains("Right") {
                commands.trigger(SoundEvent {
                    sound_anchor_entity: Some(player_entity),
                    translation: None,
                    sound_path: "sounds/walkr_br.wav".to_string(),
                    is_attached_to_local_player: is_local_player,
                });
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
                entity_id: player_state.id,
                requested_animation: RequestedAnimation::FreezeAnimation,
                networked: is_local_player,
            });
            if animation_name == "" {}
        }
        NifEventType::ResumeIdle { animation_name } => {
            trigger_request_animation(
                commands.reborrow(),
                AnimationState::Idle.into(),
                &mut player_state,
                is_local_player,
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
        }
    }
}
