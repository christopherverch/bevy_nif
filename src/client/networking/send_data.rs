//use cdda_bevy::client::animation::play_animation_local;

use super::connecting_disconnecting::{create_player, remove_player};
use super::{EntityIdToEntityMap, InterpolationSpeeds};
use bevy::prelude::*;
use bevy_renet::renet::RenetClient;
use cdda_bevy::client::actor_defs::{ActorState, UpperBodyState};
use cdda_bevy::client::defs::{
    ClientEntityId, IdToEntityMap, LocalPlayer, NetworkEventWithIdCounter, NetworkedEntity,
    PendingServerAcks, SentActionInfo,
};
use cdda_bevy::client::events::SoundSource;
use cdda_bevy::networking::{
    ClientChannel, NetworkedEntityDataSnapshot, ServerChannel, ServerMessages,
};
use cdda_bevy::serializer::SerializableTransform;
use cdda_bevy::{
    AnimationEvent, ClientMessage, NetworkEvent, NetworkEventWithId, NetworkedActorState,
    ServerAcknowledgment, ServerRelayMessage,
};
use std::time::Instant;

/// Sends the transform of the local player to the server
pub fn client_send_transform(
    local_player_q: Query<&Transform, (With<LocalPlayer>, Changed<Transform>)>,
    mut client: ResMut<RenetClient>,
) {
    if let Ok(creature_transform) = local_player_q.single() {
        let serializable_transform = SerializableTransform::from(creature_transform);
        let network_message = bincode::serialize(&serializable_transform).unwrap();
        client.send_message(ClientChannel::EntityTransforms, network_message);
    }
}

pub fn client_sync_players(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut client: ResMut<RenetClient>,
    mut lobby: ResMut<EntityIdToEntityMap>,
    mut networked_entity_q: Query<&mut NetworkedEntity>,
    mut interpolation_speeds: ResMut<InterpolationSpeeds>,
    mut id_to_entity_map: ResMut<IdToEntityMap>,
    mut actor_state_query: Query<&mut ActorState, Without<LocalPlayer>>,
    mut local_entity_id: ResMut<ClientEntityId>,
) {
    // Process server messages (player creation, removal)
    process_server_messages(
        &mut client,
        &mut lobby,
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut id_to_entity_map,
        &mut actor_state_query,
        &mut local_entity_id,
    );

    let local_entity_id = local_entity_id.0;
    // Process networked entities and update their states
    process_networked_entities(
        &mut client,
        &mut networked_entity_q,
        &mut interpolation_speeds,
        &mut lobby,
        local_entity_id,
        &id_to_entity_map,
    );
}

fn process_server_messages(
    client: &mut RenetClient,
    lobby: &mut EntityIdToEntityMap,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    mut id_to_entity_map: &mut IdToEntityMap,
    actor_state_query: &mut Query<&mut ActorState, Without<LocalPlayer>>,
    local_entity_id: &mut ClientEntityId,
) {
    while let Some(message) = client.receive_message(ServerChannel::ServerMessages) {
        let server_message = bincode::deserialize(&message).unwrap();
        match server_message {
            ServerMessages::PlayerCreate {
                entity_id,
                translation,
            } => {
                create_player(
                    entity_id,
                    translation,
                    lobby,
                    &mut id_to_entity_map,
                    commands,
                );
            }
            ServerMessages::PlayerRemove { entity_id } => {
                remove_player(entity_id, lobby, &mut id_to_entity_map, commands);
            }

            ServerMessages::StateUpdate { entity_id, state } => {
                // Use EntityIdToEntityMap to find the client-side entity for this entity Id
                if let Some(client_server_entity) = lobby.players.get(&entity_id) {
                    let client_entity = client_server_entity;

                    // Get the ActorState component for that entity from our new query parameter
                    if let Ok(mut actor_state) = actor_state_query.get_mut(*client_entity) {
                        // Apply the received animation state
                        actor_state.is_running = state.is_running;
                        actor_state.is_sneaking = state.is_sneaking;
                        actor_state.is_in_water = state.is_in_water;
                        actor_state.movement.state = state.character_state;
                        actor_state.combat.upper_body_state = state.upper_body_state;
                        actor_state.combat.weapon_type = state.weapon_type;
                        actor_state.jumping.state = state.jumping_state;
                    }
                }
            }
            ServerMessages::Welcome { entity_id } => {
                local_entity_id.0 = entity_id;
            }
        }
    }
}

fn process_networked_entities(
    client: &mut RenetClient,
    networked_entity_q: &mut Query<&mut NetworkedEntity>,
    interpolation_speeds: &mut ResMut<InterpolationSpeeds>,
    lobby: &mut EntityIdToEntityMap,
    local_entity_id: u64,
    id_to_entity_map: &IdToEntityMap,
) {
    while let Some(message) = client.receive_message(ServerChannel::NetworkedEntities) {
        let networked_entities: NetworkedEntityDataSnapshot =
            bincode::deserialize(&message).unwrap();
        if let Some(local_player_entity) = lobby.players.get(&local_entity_id) {
            update_other_player_transforms(
                local_player_entity,
                networked_entities,
                networked_entity_q,
                interpolation_speeds,
                id_to_entity_map,
            );
        }
    }
}

// Update player state and animations
fn update_other_player_transforms(
    local_player_entity: &Entity,
    received_networked_entities: NetworkedEntityDataSnapshot,
    networked_entity_q: &mut Query<&mut NetworkedEntity>,
    interpolation_speeds: &mut ResMut<InterpolationSpeeds>,
    id_to_entity_map: &IdToEntityMap,
) {
    for (networked_entity_id, networked_entity_data) in received_networked_entities.entity_data_map
    {
        if let Some(entity) = id_to_entity_map.0.get(&networked_entity_id) {
            let Ok(mut networked_entity) = networked_entity_q.get_mut(*entity) else {
                continue;
            };
            if networked_entity.entity == *local_player_entity {
                continue;
            }
            // We now have the OTHER player's transform

            // Update interpolation speed
            let elapsed = networked_entity.last_packet_time.elapsed().as_millis() as f32;
            let speed = if elapsed > 0.0 { 400.0 / elapsed } else { 10.0 };
            interpolation_speeds
                .0
                .insert(networked_entity.entity, speed);
            networked_entity.last_packet_time = Instant::now();

            // Update the entity transform from the receieved transform
            networked_entity.serializable_transform = networked_entity_data.transform;
            break;
        }
    }
}

/// This system runs on the client and sends its state to the server whenever it changes.
pub fn send_local_player_state_system(
    mut client: ResMut<RenetClient>,
    // Query for the entity that has both LocalPlayer and a changed ActorState
    query: Query<&ActorState, (With<LocalPlayer>, Changed<ActorState>)>,
) {
    // If it has changed
    if let Ok(actor_state) = query.single() {
        // Convert local ActorState into the network-ready version.
        let networked_state = NetworkedActorState::from(actor_state);

        // Wrap the state update in the ClientMessage enum.
        let message = ClientMessage::ActorStateUpdate(networked_state);

        // Serialize the message into binary data.
        let serialized_message = bincode::serialize(&message).unwrap();

        // Send the message to the server on a "reliable" channel.
        // Reliable means the network library guarantees it will arrive and in the correct order.
        client.send_message(ClientChannel::State, serialized_message);
    }
}

/// Adds a critical (not acceptable to drop) network event
pub fn add_critical_network_event(
    trigger: Trigger<NetworkEvent>,
    // take an option since observers can't be put in sets, so it will run
    // whether we're connected or not
    client_opt: Option<ResMut<RenetClient>>,
    mut action_id_counter: ResMut<NetworkEventWithIdCounter>,
    mut pending_acks: ResMut<PendingServerAcks>,
    time: Res<Time>,
    // local_player_query: Query<&Transform, With<LocalPlayer>>,
) {
    let Some(mut client) = client_opt else {
        return;
    };
    let action_id = action_id_counter.0;
    action_id_counter.0 += 1;

    let attack_action = NetworkEventWithId {
        payload: trigger.clone(),
        action_id,
    };
    match bincode::serialize(&attack_action) {
        Ok(payload) => {
            client.send_message(ClientChannel::CriticalActions, payload);
            pending_acks.0.insert(
                action_id,
                SentActionInfo {
                    action_payload: trigger.clone(),
                    time_sent: time.elapsed(),
                    retries: 0,
                },
            );
            info!("Client sent event (ID: {})", action_id);
        }
        Err(e) => error!("Failed to serialize PlayerAttack: {}", e),
    }
}

pub fn client_trigger_attack_system(
    mut client: ResMut<RenetClient>,
    mut action_id_counter: ResMut<NetworkEventWithIdCounter>,
    mut pending_acks: ResMut<PendingServerAcks>,
    time: Res<Time>,
    input: Res<ButtonInput<KeyCode>>, // Example trigger
                                      // Potentially query for local player data if needed for the attack
                                      // local_player_query: Query<&Transform, With<LocalPlayer>>,
) {
    if input.just_pressed(KeyCode::KeyA) {
        // 'A' for Attack
        // let player_transform = local_player_query.single(); // Get player transform

        let action_id = action_id_counter.0;
        action_id_counter.0 += 1;

        let attack_action = NetworkEventWithId {
            payload: NetworkEvent::TestingString("testing critical action".to_string()),
            action_id,
        };

        match bincode::serialize(&attack_action) {
            Ok(payload) => {
                client.send_message(ClientChannel::CriticalActions, payload);
                pending_acks.0.insert(
                    action_id,
                    SentActionInfo {
                        action_payload: attack_action.payload, // Clone for potential retry
                        time_sent: time.elapsed(),
                        retries: 0,
                    },
                );
                info!("Client sent PlayerAttack (ID: {})", action_id);
            }
            Err(e) => error!("Failed to serialize PlayerAttack: {}", e),
        }
    }
}
pub fn client_handle_server_acks_system(
    mut client: ResMut<RenetClient>,
    mut pending_acks: ResMut<PendingServerAcks>,
) {
    while let Some(message_bytes) = client.receive_message(ServerChannel::CriticalAcknowledgments) {
        match bincode::deserialize::<ServerAcknowledgment>(&message_bytes) {
            Ok(ServerAcknowledgment::ActionConfirmed {
                original_action_id,
                server_validation_passed,
                reason_if_failed,
            }) => {
                if pending_acks.0.remove(&original_action_id).is_some() {
                    if server_validation_passed {
                        info!(
                            "Server confirmed action ID: {}. Relay should occur.",
                            original_action_id
                        );
                        // Action is now "fully confirmed" by server for relay
                    } else {
                        warn!(
                            "Server REJECTED action ID: {}. Reason: {:?}",
                            original_action_id, reason_if_failed
                        );
                        // !!! Important: Client might need to revert its local prediction here !!!
                        // E.g., if an attack animation played, stop it or play a "fizzle" effect.
                    }
                } else {
                    warn!(
                        "Received ACK for unknown or already processed action ID: {}",
                        original_action_id
                    );
                }
            }
            Err(e) => error!("Failed to deserialize ServerAcknowledgment: {}", e),
        }
    }
}
pub fn client_handle_relayed_actions_system(
    mut client: ResMut<RenetClient>,
    mut commands: Commands,
    lobby: Res<EntityIdToEntityMap>,
    mut remote_player_query: Query<&mut ActorState, Without<LocalPlayer>>,
) {
    while let Some(message_bytes) = client.receive_message(ServerChannel::CriticalRelay) {
        match bincode::deserialize::<ServerRelayMessage>(&message_bytes) {
            Ok(ServerRelayMessage::RelayClientAction {
                performing_client_id,
                action,
            }) => {
                // This client is NOT `performing_client_id`

                info!(
                    "Received relayed action from client {}: {:?}",
                    performing_client_id, action,
                );
                match action.payload {
                    NetworkEvent::Animation(animation_event) => {
                        // Find the entity that this event belongs to
                        if let Some(target_entity) =
                            lobby.players.get(&performing_client_id).map(|e| e)
                        {
                            // Get its ActorState component
                            if let Ok(mut actor_state) = remote_player_query.get_mut(*target_entity)
                            {
                                // Apply the event
                                match animation_event {
                                    AnimationEvent::JumpStarted => {
                                        // This tells the animation system into playing the jump intro
                                        actor_state.jumping.handled_jump_animation = false;
                                    }
                                    AnimationEvent::Landed => {
                                        // This tells the animation system into playing the land animation
                                        actor_state.jumping.handled_landing_animation = false;
                                    }
                                    AnimationEvent::AttackStarted { attack_type } => {
                                        // This kicks off the attack state machine on the remote client
                                        actor_state.combat.upper_body_state =
                                            UpperBodyState::AttackWindUp;
                                        actor_state.combat.attack_type = attack_type;
                                        actor_state.combat.ready_to_hit = true; // Ensure hit logic can run
                                    }
                                }
                            }
                        }
                    }

                    NetworkEvent::Hit(mut triggered_hit) => {
                        triggered_hit.networked = false;
                        commands.trigger(triggered_hit);
                    }
                    NetworkEvent::Sound(mut sound) => {
                        if sound.sound_source == SoundSource::LocalPlayer {
                            if let Some(target_entity) =
                                lobby.players.get(&performing_client_id).map(|e| e)
                            {
                                sound.sound_source = SoundSource::Attached(*target_entity);
                            }
                        }
                        commands.trigger(sound);
                    }
                    NetworkEvent::SetAnimSpeedSeek(mut anim_speed_event) => {
                        anim_speed_event.networked = false;
                        commands.trigger(anim_speed_event);
                    }
                    _ => {}
                }
                // Apply the effects of this action to this client's world view
                // E.g., if action is PlayerAttack, find the target, apply damage, play effects.
            }
            Err(e) => error!("Failed to deserialize ServerRelayMessage: {}", e),
        }
    }
}
