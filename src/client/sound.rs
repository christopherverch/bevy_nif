use bevy::prelude::*;
use bevy_kira_audio::{Audio, AudioControl, SpatialAudioEmitter};
use cdda_bevy::client::events::SoundEvent;

pub fn generate_sound(
    trigger: Trigger<SoundEvent>,
    mut commands: Commands,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
) {
    let generated_sound = audio
        .play(asset_server.load(trigger.sound_path.clone()))
        .with_volume(0.1)
        .handle();
    let sound_entity: Option<Entity>;
    if let Some(sound_anchor_entity) = trigger.sound_anchor_entity {
        sound_entity = Some(sound_anchor_entity);
    } else {
        let detached_entity = commands
            .spawn((
                // Spawn as a tuple to include multiple components
                Transform {
                    // if we weren't given an entity, the translation should always be Some
                    translation: trigger.translation.unwrap(),
                    rotation: Quat::IDENTITY,
                    scale: Vec3::splat(1.0),
                },
            ))
            .id();
        sound_entity = Some(detached_entity);
    }
    if !trigger.is_attached_to_local_player {
        commands
            .entity(sound_entity.unwrap())
            .insert(SpatialAudioEmitter {
                instances: vec![generated_sound],
            });
    }
}
