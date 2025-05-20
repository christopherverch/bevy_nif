use bevy::prelude::*;
use cdda_bevy::{
    AnimationState, TriggeredAction,
    client::{
        defs::{CreatureStats, IsThirdPerson, LocalPlayer, PlayerState},
        events::{RequestAnimation, SoundEvent, TakeHit},
    },
};

use crate::send_data::queue_network_triggered_action;
pub fn actor_take_hit(
    trigger: Trigger<TakeHit>,
    mut creature_state_q: Query<(Entity, &mut CreatureStats)>,
    mut commands: Commands,
    mut local_player_q: Query<(Entity, &mut PlayerState), With<LocalPlayer>>,
    mut is_third_person: ResMut<IsThirdPerson>,
) {
    let Some((entity, mut creature_stats)) = creature_state_q
        .iter_mut()
        .find(|(_, creature_stats)| creature_stats.id == trigger.creature_id)
    else {
        return;
    };
    creature_stats.health = creature_stats.health - trigger.damage;
    if creature_stats.health <= 0.0 {
        commands.trigger(RequestAnimation {
            entity,
            requested_animation: AnimationState::Death.into(),
        });
        let (local_player_entity, _) = local_player_q.single().unwrap();
        //if it's the local player that died
        if local_player_entity == entity {
            if !is_third_person.0 {
                is_third_person.0 = !is_third_person.0;
            }
            commands
                .entity(local_player_entity)
                .insert(Visibility::Visible);
        }
    }
    if !trigger.is_from_network {
        //TODO:: don't put this in the local player's player_state because it has nothing to do with
        //their state, it's just for networking
        let (_local_player_entity, mut local_player_state) = local_player_q.single_mut().unwrap();
        queue_network_triggered_action(
            &mut local_player_state,
            TriggeredAction::TakeHit {
                creature_id: creature_stats.id,
                damage: trigger.damage,
            },
        );
    }

    println!("player health: {}", creature_stats.health);
    commands.trigger(SoundEvent("sounds/body hit.wav".to_string()));
    if trigger.damage > 12.0 {
        commands.trigger(RequestAnimation {
            entity,
            requested_animation: AnimationState::Knockdown.into(),
        });
    }
}
