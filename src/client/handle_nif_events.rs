use bevy::prelude::*;
use bevy_nif::nif_animation::{NifEvent, NifEventType};
use tes3mp_rust::{
    defs::{CreatureStats, LocalPlayer},
    events::{SoundEvent, SoundSource},
};

pub fn handle_nif_trigger(
    trigger: Trigger<NifEvent>,
    mut commands: Commands,
    local_player_q: Single<&CreatureStats, With<LocalPlayer>>,
) {
    let local_player = local_player_q.into_inner();
    info!("nif event triggered: {:?} ", trigger.event_type,);
    let is_local_player = local_player.nif_animator_link == Some(trigger.entity);
    match &trigger.event_type {
        NifEventType::SoundGen { sound_name } => {
            let sound_source = if is_local_player {
                SoundSource::LocalPlayer
            } else {
                SoundSource::Attached(trigger.entity)
            };
            if sound_name.contains("Left") {
                commands.trigger(SoundEvent {
                    sound_source,
                    sound_path: "sounds/walkl_br.wav".to_string(),
                });
            } else if sound_name.contains("Right") {
                commands.trigger(SoundEvent {
                    sound_source,
                    sound_path: "sounds/walkr_br.wav".to_string(),
                });
            }
        }
    }
}
