use bevy::prelude::*;
use bevy_kira_audio::{Audio, AudioControl, SpatialAudioEmitter, SpatialRadius};
use tes3mp_rust::events::{SoundEvent, SoundSource};

/// Reacts to SoundEvent triggers and spawns a sound either attached to the player, an entity or detached
pub fn generate_sound(
    event: On<SoundEvent>,
    mut commands: Commands,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    spatial_emitters_q: Query<&SpatialAudioEmitter>,
) {
    // Set the sound emitting entity to either the one given by the trigger, or a new entity we
    // spawn with the sound trigger's translation
    let sound_source_entity = match event.sound_source {
        SoundSource::Attached(entity) => Some(entity),
        SoundSource::Located(translation) => {
            let detached_entity = commands
                .spawn((Transform {
                    translation,
                    rotation: Quat::IDENTITY,
                    scale: Vec3::splat(1.0),
                },))
                .id();
            Some(detached_entity)
        }
        // Only play the audio spatially if it's not the local player making the sound
        SoundSource::LocalPlayer => None,
    };
    if let Some(sound_entity) = sound_source_entity {
        // If the attached entity doesn't have spatial audio components, add them
        if spatial_emitters_q.get(sound_entity).is_err() {
            commands.entity(sound_entity).insert((
                SpatialAudioEmitter::default(),
                SpatialRadius { radius: 12.0 },
            ));
        }
        audio
            .play(asset_server.load(event.sound_path.clone()))
            .with_emitter(sound_entity)
            .with_volume(-10.0);
    } else {
        dbg!("play");
        audio
            .play(asset_server.load(event.sound_path.clone()))
            .with_volume(-10.0);
    }
}
