use crate::{animation::NifAnimatorLink, defs::LocalPlayer};
use bevy::prelude::*;
use bevy_nif::nif_animation::{NifEvent, NifEventType};
use common::events::{SoundEvent, SoundSource};

pub fn handle_nif_event(
    event: On<NifEvent>,
    mut commands: Commands,
    local_player_q: Single<&NifAnimatorLink, With<LocalPlayer>>,
) {
    let local_player = local_player_q.into_inner();
    let is_local_player = local_player.0 == event.entity;
    match &event.event_type {
        NifEventType::SoundGen { sound_name } => {
            let sound_source = if is_local_player {
                SoundSource::LocalPlayer
            } else {
                SoundSource::Attached(event.entity)
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
            } else if sound_name.contains("Land") {
                // These are everywhere, we just play it manually when we land to replicate
                // morrowind
            }
        }
    }
}
