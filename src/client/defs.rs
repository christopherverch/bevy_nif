use crate::AnimationState;
use crate::AnimationWeights;
use bevy::prelude::*;
use std::collections::HashMap;
use std::time::Instant;
#[derive(Resource, Component)]
pub struct PlayerState {
    pub id: u64,
    pub transform: Transform,
    pub last_packet_time: Instant,
    pub started_attacking: Instant,
    pub is_attacking: bool,
    pub is_ready_weapon: bool,
    pub animation_weights: AnimationWeights,
    pub current_animation: AnimationState,
    pub secondary_animation: AnimationState,
}
impl Default for PlayerState {
    fn default() -> Self {
        PlayerState {
            id: 0,
            transform: Transform::default(),
            last_packet_time: Instant::now(),
            current_animation: AnimationState::Idle,
            secondary_animation: AnimationState::Idle,
            started_attacking: Instant::now(),
            is_attacking: false,
            is_ready_weapon: false,
            animation_weights: AnimationWeights {
                idle: 1.0,
                run: 0.0,
                swing: 0.0,
            },
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, States, Default, Hash)]
pub enum SpawnSceneState {
    #[default]
    Spawning,
    Spawned,
}
#[derive(Default, Resource)]
pub struct Animations {
    pub animations: Vec<AnimationNodeIndex>,
    pub graph: Handle<AnimationGraph>,
}
#[derive(Component)]
pub struct Animator {
    pub graph: Handle<AnimationGraph>,
    pub animations: Vec<AnimationNodeIndex>,
}
#[derive(PartialEq)]
pub enum MaskGroup {
    ARMS,
    TORSO,
    // Add more as needed
}
pub enum AnimationNodeConfig {
    Standard {
        clip: Handle<AnimationClip>,
        speed: f32,
    },
    Masked {
        clip: Handle<AnimationClip>,
        mask: u64,
        speed: f32,
    },
}
#[derive(Default, Resource, Debug)]
pub struct SceneEntitiesByName(pub HashMap<(String, u64), Entity>);
#[derive(Resource)]
pub struct Pause(pub bool);
#[derive(Clone, Copy, Debug, Eq, PartialEq, States, Default, Hash)]
pub enum GameState {
    #[default]
    MainMenu,
    Connecting,
    Offline,
    Connected,
}
#[derive(Component)]
pub struct PlayerRoot;
#[derive(Component)]
pub struct NetworkedEntity(pub Entity);
#[derive(Component)]
pub struct Player;
#[derive(Debug, Resource)]
pub struct CurrentClientId(pub u64);

#[derive(Component)]
pub struct Speed(pub f32);
#[derive(Resource, Default)]
pub struct IsThirdPerson(pub bool);
#[derive(Component)]
pub struct NeedsSetup {
    pub id: u64,
    pub player_transform: Transform,
}
#[derive(Component)]
pub struct PlayerCamera;
#[derive(Component)]
pub struct Weapon;
#[derive(Component, Default, Resource)]
pub struct LocalPlayer(pub Option<Entity>);
#[derive(Resource)]
pub struct ServerIP(pub String);
#[derive(Resource, Default)]
pub struct PlayerInput {
    pub move_direction: Vec2,
    pub jump: bool,
    // Add other input states as needed
}
#[derive(Component)]
pub struct SceneName {
    pub scene_name: String,
    pub id: u64,
}
#[derive(Component)]
pub struct AnimateableEntity;
