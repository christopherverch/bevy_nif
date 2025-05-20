use bevy::prelude::*;
use cdda_bevy::client::{defs::MOVEMENT_VOLUME, events::SoundEvent};

use crate::wav_loader::MorrowindDecodedWav;
pub fn generate_sound(
    trigger: Trigger<SoundEvent>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let sound_handle: Handle<MorrowindDecodedWav> = asset_server.load(&trigger.0);
    commands.spawn((
        // Spawn as a tuple to include multiple components
        AudioPlayer(sound_handle),
        PlaybackSettings {
            volume: MOVEMENT_VOLUME,
            ..default()
        },
    ));
}
