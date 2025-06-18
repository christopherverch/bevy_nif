use bevy::prelude::*;
use bevy_nif::nif_animation::{BlendMask, NifEventType};
use client::{
    actor_defs::{ActorState, CharacterState, JumpingStateKind, UpperBodyState},
    events::{SetAnimSpeedSeekEvent, SoundEvent, TriggerHit},
};
use serde::{Deserialize, Serialize};
pub mod client;
pub mod networking;
pub mod serializer;
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RepeatAnim {
    Never,
    Forever,
    Count(u32),
}
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AnimationConfig {
    pub name: String,
    pub repeat: RepeatAnim,
    pub priority: u8,
    pub mask: BlendMask,
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
    pub weapon_type: i32,
    pub jumping_state: JumpingStateKind,
}
// Helper function to convert from your full ActorState to the lean NetworkedActorState
impl From<&ActorState> for NetworkedActorState {
    fn from(actor_state: &ActorState) -> Self {
        Self {
            is_running: actor_state.is_running,
            is_sneaking: actor_state.is_sneaking,
            is_in_water: actor_state.is_in_water,
            character_state: actor_state.movement.state,
            upper_body_state: actor_state.combat.upper_body_state,
            weapon_type: actor_state.combat.weapon_type,
            jumping_state: actor_state.jumping.state,
        }
    }
}

/// An enum of all possible messages a Client can send to the Server.
#[derive(Debug, Serialize, Deserialize, Component)]
pub enum ClientMessage {
    // We can add more later, like Jump or Attack events.
    StateUpdate(NetworkedActorState),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkEventWithId {
    pub action_id: u64,
    pub payload: NetworkEvent, // The specific type of action
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
        // might also send initial attack strength if relevant
        // attack_strength: f32,
    },
    // HitReaction { ... },
    // Died { ... },
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
        performing_client_id: u64,  // The client who was authoritative
        action: NetworkEventWithId, // The original action (or a version of it)
    },
}
