use std::{collections::HashMap, time::Duration};

use bevy_ecs::{component::Component, entity::Entity, event::Event, resource::Resource};
use bevy_math::Vec3;
use bevy_nif::nif_animation::NifEventType;
use bevy_renet::renet::{ChannelConfig, ConnectionConfig, SendType};
use esm_parser_lib::types::enums::WeaponType;
use serde::{Deserialize, Serialize};

use common::{
    EntityId,
    events::{SetAnimSpeedSeekEvent, SoundEvent, TriggerHit},
    mechanics::{CharacterController, CharacterState, JumpingStateKind, UpperBodyState},
};
use serializer::SerializableTransform;

pub const PRIVATE_KEY: &[u8; bevy_renet::netcode::NETCODE_KEY_BYTES] =
    b"an example very very secret key."; // 32-bytes
pub const PROTOCOL_ID: u64 = 7;

pub enum ClientChannel {
    EntityTransforms,
    CriticalActions,
    State,
}
pub enum ServerChannel {
    CriticalAcknowledgments,
    CriticalRelay,
    ServerMessages,
    NetworkedEntities,
    StateUpdates,
}

#[derive(Debug, Default, Component)]
pub struct Velocity(pub Vec3);

#[derive(Debug, Serialize, Deserialize, Component)]
pub enum ServerMessages {
    PlayerCreate {
        entity_id: EntityId,
        translation: [f32; 3],
    },
    PlayerRemove {
        entity_id: EntityId,
    },

    /// Relays the animation state of a specific player.
    StateUpdate {
        /// The ID of the entity whose state this is.
        entity_id: EntityId,
        /// The new state of that entity.
        state: NetworkedActorState,
    },
    /// Tells the player what their server-assigned entity id is
    Welcome {
        entity_id: EntityId,
    },
}
#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkedEntityData {
    pub transform: SerializableTransform,
}
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct NetworkedEntityDataSnapshot {
    pub entity_data_map: HashMap<EntityId, NetworkedEntityData>,
}
impl From<ClientChannel> for u8 {
    fn from(channel_id: ClientChannel) -> Self {
        match channel_id {
            ClientChannel::EntityTransforms => 0,
            ClientChannel::CriticalActions => 1,
            ClientChannel::State => 2,
        }
    }
}

impl ClientChannel {
    pub fn channels_config() -> Vec<ChannelConfig> {
        vec![
            ChannelConfig {
                channel_id: Self::EntityTransforms.into(),
                max_memory_usage_bytes: 5 * 1024 * 1024,
                // We don't care if positions jump a little for networked entities
                send_type: SendType::Unreliable,
            },
            ChannelConfig {
                channel_id: Self::CriticalActions.into(),
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::from_millis(250),
                },
            },
            ChannelConfig {
                channel_id: Self::State.into(),
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::from_millis(250),
                },
            },
        ]
    }
}

impl From<ServerChannel> for u8 {
    fn from(channel_id: ServerChannel) -> Self {
        match channel_id {
            ServerChannel::NetworkedEntities => 0,
            ServerChannel::ServerMessages => 1,
            ServerChannel::CriticalAcknowledgments => 2,
            ServerChannel::CriticalRelay => 3,
            ServerChannel::StateUpdates => 4,
        }
    }
}

impl ServerChannel {
    pub fn channels_config() -> Vec<ChannelConfig> {
        vec![
            ChannelConfig {
                channel_id: Self::NetworkedEntities.into(),
                max_memory_usage_bytes: 10 * 1024 * 1024,
                send_type: SendType::Unreliable,
            },
            ChannelConfig {
                channel_id: Self::ServerMessages.into(),
                max_memory_usage_bytes: 10 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::from_millis(200),
                },
            },
            ChannelConfig {
                channel_id: Self::CriticalAcknowledgments.into(),
                max_memory_usage_bytes: 10 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::from_millis(200),
                },
            },
            ChannelConfig {
                channel_id: Self::CriticalRelay.into(),
                max_memory_usage_bytes: 10 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::from_millis(200),
                },
            },
            ChannelConfig {
                channel_id: Self::StateUpdates.into(),
                max_memory_usage_bytes: 10 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::from_millis(200),
                },
            },
        ]
    }
}

pub fn connection_config() -> ConnectionConfig {
    ConnectionConfig {
        available_bytes_per_tick: 1024 * 1024,
        client_channels_config: ClientChannel::channels_config(),
        server_channels_config: ServerChannel::channels_config(),
    }
}

/// A component containing only the state essential for animation replication.
/// This is what gets sent over the network.
#[derive(Debug, Clone, Serialize, Deserialize, Component)]
pub struct NetworkedActorState {
    pub is_running: bool,
    pub is_sneaking: bool,
    pub is_in_water: bool,
    pub character_state: CharacterState,
    pub upper_body_state: UpperBodyState,
    pub weapon_type: WeaponType,
    pub jumping_state: JumpingStateKind,
}
// Helper function to convert from full ActorState to the smaller NetworkedActorState
impl From<&CharacterController> for NetworkedActorState {
    fn from(actor_state: &CharacterController) -> Self {
        Self {
            is_running: actor_state.is_running(),
            is_sneaking: actor_state.is_sneaking(),
            is_in_water: false,
            character_state: actor_state.movement.state,
            upper_body_state: actor_state.combat.upper_body,
            weapon_type: actor_state.combat.weapon_type,
            jumping_state: actor_state.jumping.state,
        }
    }
}

/// An enum of all possible messages a Client can send to the Server.
#[derive(Debug, Serialize, Deserialize, Component)]
pub enum ClientMessage {
    // add more later, like Jump or Attack events.
    ActorStateUpdate(NetworkedActorState),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkEventWithId {
    pub action_id: u64,
    pub payload: NetworkEvent,
}
#[derive(Event, Debug, Serialize, Deserialize, Clone)]
pub enum NetworkEvent {
    TestingString(String),
    SetAnimSpeedSeek(SetAnimSpeedSeekEvent),
    Hit(TriggerHit),
    Sound(SoundEvent),
    Animation(AnimationEvent),
    Nif(NifEventType),
}
#[derive(Event, Debug, Serialize, Deserialize, Clone)]
pub enum AnimationEvent {
    JumpStarted,
    /// A character has landed on the ground.
    Landed,
    /// A character has started a weapon attack.
    AttackStarted {
        // We need to know the attack type to play the right animation (e.g., slash, thrust)
        attack_type: String,
    },
}
impl NetworkEventWithId {
    pub fn animation(action_id: u64, animation_event: AnimationEvent) -> Self {
        Self {
            action_id,
            payload: NetworkEvent::Animation(animation_event),
        }
    }
    pub fn freeze_animation(action_id: u64, animation_speed_event: SetAnimSpeedSeekEvent) -> Self {
        Self {
            action_id,
            payload: NetworkEvent::SetAnimSpeedSeek(animation_speed_event),
        }
    }
    pub fn hit(action_id: u64, trigger_hit: TriggerHit) -> Self {
        Self {
            action_id,
            payload: NetworkEvent::Hit(trigger_hit),
        }
    }
    pub fn sound(action_id: u64, sound_event: SoundEvent) -> Self {
        Self {
            action_id,
            payload: NetworkEvent::Sound(sound_event),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerAcknowledgment {
    ActionConfirmed {
        original_action_id: u64,
        server_validation_passed: bool, // Did it pass basic server checks?
        reason_if_failed: Option<String>,
    },
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerRelayMessage {
    RelayClientAction {
        performing_entity_id: u64,  // The entity performing this action
        action: NetworkEventWithId, // The original action (or a version of it)
    },
}
#[derive(Debug, Default, Resource)]
pub struct EntityIdToEntityMap {
    pub players: HashMap<EntityId, Entity>,
}
