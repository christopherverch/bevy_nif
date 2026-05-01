use crate::animation::{AnimationConfig, RepeatAnim};
use crate::{
    defs::{CreatureStats, IdToEntityMap, IsThirdPerson, LocalPlayer},
    events::RequestAnimation,
};
use bevy::prelude::*;
use bevy_nif::nif_animation::BlendMask;
use common::{ActorState, SoundEvent, SoundSource, TriggerHit};
use networking::NetworkEvent;

pub fn apply_hit(
    event: On<TriggerHit>,
    mut creature_state_q: Query<(&mut CreatureStats, &mut Transform)>,
    mut commands: Commands,
    local_player_q: Query<Entity, With<LocalPlayer>>,
    id_to_entity_map: Res<IdToEntityMap>,
    mut is_third_person: ResMut<IsThirdPerson>,
) {
    if event.networked {
        commands.trigger(NetworkEvent::Hit(event.clone()));
    }
    let Some(_attacker_entity) = id_to_entity_map.0.get(&event.attacker_id) else {
        return;
    };
    let Some(receiver_entity) = id_to_entity_map.0.get(&event.receiver_id) else {
        return;
    };
    let Ok((mut receiver_creature_stats, _receiver_transform)) =
        creature_state_q.get_mut(*receiver_entity)
    else {
        return;
    };
    let local_player_entity = local_player_q.single().unwrap();
    let local_player_was_hit = local_player_entity == *receiver_entity;

    receiver_creature_stats.health -= event.damage;
    if receiver_creature_stats.health <= 0.0 {
        commands.trigger(RequestAnimation {
            skeleton_id: event.receiver_id,
            animation_config: AnimationConfig {
                name: "death4".to_string(),
                repeat: RepeatAnim::Never,
                priority: 10,
                mask: BlendMask::all(),
            },
            networked: event.networked,
        });
        //if it's the local player that died
        if local_player_was_hit {
            if !is_third_person.0 {
                is_third_person.0 = !is_third_person.0;
            }
            commands
                .entity(local_player_entity)
                .insert(Visibility::Visible);
        }
    }

    println!("player health: {}", receiver_creature_stats.health);
    let sound_source = if local_player_was_hit {
        SoundSource::LocalPlayer
    } else {
        SoundSource::Attached(*receiver_entity)
    };
    commands.trigger(SoundEvent {
        sound_source,
        sound_path: "sounds/body hit.wav".to_string(),
    });
    if event.damage > 12.0 {
        println!("requesting knockdown for {}", receiver_entity);
        commands.trigger(RequestAnimation {
            skeleton_id: event.receiver_id,
            animation_config: AnimationConfig {
                name: "knockdown".to_string(),
                repeat: RepeatAnim::Never,
                priority: 10,
                mask: BlendMask::all(),
            },
            networked: event.networked,
        });
    }
}

// This is a conceptual example
pub fn get_current_speed(actor_state: &ActorState) -> f32 {
    let base = 2.0;

    if actor_state.is_running {
        // Return a higher value for running
        return base * 1.5; // Or whatever multiplier feels right
    } else {
        // Return a lower value for walking
        return base;
    }
    // This would also be modified by magic effects, skills, etc.
}
