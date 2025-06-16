use bevy::prelude::*;
use bevy_nif::nif_animation::{BlendMask, NifAnimator, NifEvent, NifEventType};
use cdda_bevy::{
    AnimationConfig, RepeatAnim,
    client::{
        defs::{CreatureStats, IdToEntityMap, LocalPlayer, LocomotionState, NifAnimatorLink},
        events::{RequestAnimation, SetAnimSpeedSeekEvent, SoundEvent},
    },
};

pub fn handle_nif_trigger(
    trigger: Trigger<NifEvent>,
    mut commands: Commands,
    local_player_q: Query<&LocalPlayer>,
    mut animator_q: Query<&NifAnimator>,
    mut player_query: Query<(Entity, &NifAnimatorLink, &mut CreatureStats)>,
    id_to_entity_map: Res<IdToEntityMap>,
) {
    let Some(entity_to_animate) = id_to_entity_map.0.get(&trigger.skeleton_id) else {
        return;
    };
    let Ok((player_entity, nif_animator_link, mut creature_stats)) =
        player_query.get_mut(*entity_to_animate)
    else {
        return;
    };
    let Ok(nif_animator) = animator_q.get_mut(nif_animator_link.0) else {
        return;
    };
    info!(
        "nif event triggered: {:?} {}",
        trigger.event_type, nif_animator.skeleton_id
    );
    let is_local_player = local_player_q.get(player_entity).is_ok();
    match &trigger.event_type {
        NifEventType::Other { event } => {
            if event.contains("SoundGen") {
                let sound_name = event;
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
                }
            } else {
            }
        }
        NifEventType::LoopAnimation { animation_name } => {
            commands.trigger(RequestAnimation {
                skeleton_id: creature_stats.id,
                animation_config: AnimationConfig {
                    name: format!("{}_loop", animation_name),
                    repeat: RepeatAnim::Forever,
                    priority: 0,
                    mask: BlendMask::all(),
                },
                networked: false,
            });
        }
        NifEventType::FreezeAnimation { animation_name } => {
            commands.trigger(SetAnimSpeedSeekEvent {
                skeleton_id: creature_stats.id,
                animation: animation_name.to_string(),
                speed: 0.0,
                seek_to: None,
                networked: false,
            });
        }
        NifEventType::ResumeIdle { animation_name } => {
            commands.trigger(RequestAnimation {
                skeleton_id: creature_stats.id,
                animation_config: AnimationConfig {
                    name: animation_name.to_string(),
                    repeat: RepeatAnim::Forever,
                    priority: 0,
                    mask: BlendMask::all(),
                },
                networked: false,
            });
        }
    }
}
