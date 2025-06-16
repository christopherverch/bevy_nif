use bevy::prelude::*;
use bevy_nif::nif_animation::BlendMask;
use client::{
    defs::NetworkAnimEvent,
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

/*
#[derive(Component, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RequestedAnimation {
    Common(AnimationState),
    SwingInactive,
    SwingReady,
    SwingCharging,
    SwingReleasing,
    SwingReturning,
    LoopMovement { animation: String },
    FreezeAnimEvent,
    ResumeAnimation,
}

impl From<RequestedAnimation> for AnimationState {
    fn from(req: RequestedAnimation) -> Self {
        match req {
            RequestedAnimation::Common(state) => state,
            RequestedAnimation::SwingInactive
            | RequestedAnimation::SwingReady
            | RequestedAnimation::SwingCharging
            | RequestedAnimation::SwingReleasing
            | RequestedAnimation::SwingReturning => AnimationState::Swinging,
            RequestedAnimation::LoopMovement { .. } => AnimationState::Undefined,
            RequestedAnimation::FreezeAnimEvent { .. } => AnimationState::Undefined,
            RequestedAnimation::ResumeAnimation => AnimationState::Undefined,
        }
    }
}

impl From<AnimationState> for RequestedAnimation {
    fn from(state: AnimationState) -> Self {
        RequestedAnimation::Common(state)
    }
}
*/
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkEventWithId {
    pub action_id: u64,
    pub payload: NetworkEvent, // The specific type of action
}
#[derive(Event, Debug, Serialize, Deserialize, Clone)]
pub enum NetworkEvent {
    TestingString(String),
    Animation(NetworkAnimEvent),
    SetAnimSpeedSeek(SetAnimSpeedSeekEvent),
    Hit(TriggerHit),
    Sound(SoundEvent),
}
impl NetworkEventWithId {
    pub fn animation(action_id: u64, animation_event: NetworkAnimEvent) -> Self {
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
