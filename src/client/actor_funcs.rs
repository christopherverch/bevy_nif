use bevy::prelude::*;
use cdda_bevy::{
    AnimationState, NetworkEvent,
    client::{
        defs::{CreatureStats, IdToEntityMap, IsThirdPerson, LocalPlayer, PlayerState},
        events::{RequestAnimation, SoundEvent, TriggerHit},
    },
};

pub fn apply_hit(
    trigger: Trigger<TriggerHit>,
    mut creature_state_q: Query<(&mut CreatureStats, &mut Transform)>,
    mut commands: Commands,
    local_player_q: Query<(Entity, &mut PlayerState), With<LocalPlayer>>,
    id_to_entity_map: Res<IdToEntityMap>,
    mut is_third_person: ResMut<IsThirdPerson>,
) {
    if trigger.networked {
        commands.trigger(NetworkEvent::Hit(trigger.clone()));
    }
    let Some(_attacker_entity) = id_to_entity_map.0.get(&trigger.attacker_id) else {
        return;
    };
    let Some(receiver_entity) = id_to_entity_map.0.get(&trigger.receiver_id) else {
        return;
    };
    let Ok((mut receiver_creature_stats, _receiver_transform)) =
        creature_state_q.get_mut(*receiver_entity)
    else {
        return;
    };
    let (local_player_entity, _) = local_player_q.single().unwrap();
    let local_player_was_hit = local_player_entity == *receiver_entity;

    receiver_creature_stats.health -= trigger.damage;
    if receiver_creature_stats.health <= 0.0 {
        commands.trigger(RequestAnimation {
            entity_id: trigger.receiver_id,
            requested_animation: AnimationState::Death.into(),
            networked: trigger.networked,
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
    commands.trigger(SoundEvent {
        sound_anchor_entity: Some(*receiver_entity),
        translation: None,
        sound_path: "sounds/body hit.wav".to_string(),
        is_attached_to_local_player: local_player_was_hit,
    });
    if trigger.damage > 12.0 {
        println!("requesting knockdown for {}", receiver_entity);
        commands.trigger(RequestAnimation {
            entity_id: trigger.receiver_id,
            requested_animation: AnimationState::Knockdown.into(),
            networked: trigger.networked,
        });
    }
}
