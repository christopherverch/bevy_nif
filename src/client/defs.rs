use crate::serializer::PlayerStateNetwork;
use crate::{AnimationState, NetworkEvent, RequestedAnimation};
use bevy::prelude::*;
use bevy_ecs::component::HookContext;
use bevy_ecs::world::DeferredWorld;
use bevy_nif::nif::animation::NifEventType;
use esm_parser_lib::{EsmReader, LandTexture, Landscape};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Resource, Component)]
pub struct PlayerState {
    pub id: u64,
    pub transform: Transform,
    pub last_packet_time: Instant,
    pub started_attacking: Instant,
    pub is_attacking: bool,
    pub is_moving: bool,
    pub is_ready_weapon: bool,
    pub attack_state: AttackState,
    pub current_animation: AnimationState,
    pub secondary_animation: AnimationState,
    pub requested_primary_animation: Option<RequestedAnimation>,
    pub requested_secondary_animation: Option<RequestedAnimation>,
}
impl Default for PlayerState {
    fn default() -> Self {
        PlayerState {
            id: 0,
            transform: Transform::default(),
            last_packet_time: Instant::now(),
            started_attacking: Instant::now(),
            is_attacking: false,
            is_moving: false,
            is_ready_weapon: false,
            attack_state: AttackState::Inactive,
            current_animation: AnimationState::Idle,
            secondary_animation: AnimationState::Idle,
            requested_primary_animation: None,
            requested_secondary_animation: None,
        }
    }
}
impl PlayerState {
    pub fn update_from_network(&mut self, net: PlayerStateNetwork) {
        self.transform = net.transform.into();
        self.attack_state = net.attack_state;
    }
}
//we want this to be its own component so we can add it to actors, creatures, etc
#[derive(Component, Clone, Debug, Serialize, Deserialize, Resource)]
#[component(on_add = add_creature_to_id_map)]
pub struct CreatureStats {
    pub id: u64,
    pub health: f32,
}
impl CreatureStats {
    pub fn new(id: u64) -> Self {
        CreatureStats {
            id,
            health: 50.0, // auto-set
        }
    }
}
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum AttackState {
    Inactive,
    Ready,
    Charging,
    ChargeImmediateRelease,
    Releasing,
    Returning,
    FrozenBack,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, States, Default, Hash)]
pub enum SpawnSceneState {
    #[default]
    Spawning,
    Spawned,
}
#[derive(Component, Debug)]
pub struct FirstPersonMeshEntityLinks(pub Vec<Entity>);
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AnimationEvent {
    NifAnimationEvent {
        event_type: NifEventType,
    },
    RequestedAnimationEvent {
        requested_animation: RequestedAnimation,
        target_id: u64,
    },
}

/*impl From<NifEventType> for AnimationEvent {
    fn from(event: NifEventType) -> Self {
        match event {
            NifEventType::PlaySound { sound_name } => AnimationEvent::SoundGen { sound_name },
            NifEventType::ResumeIdle { animation_name } => {
                AnimationEvent::ResumeIdle { animation_name }
            }
            NifEventType::FreezeAnimation { animation_name } => {
                AnimationEvent::FreezeAnimation { animation_name }
            }
            NifEventType::LoopAnimation { animation_name } => {
                AnimationEvent::LoopAnimation { animation_name }
            }
        }
    }
}*/
#[derive(Default, Resource)]
pub struct Animations {
    pub animations: Vec<AnimationNodeIndex>,
    pub graph: Handle<AnimationGraph>,
}
#[derive(Component)]
pub struct NifAnimatorLink(pub Entity);
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
pub struct TestingComponent;
#[derive(Component)]
pub struct PlayerEntityLink(pub Option<Entity>);
#[derive(Component)]
pub struct NetworkedEntity(pub Entity);
#[derive(Resource, Default, Debug)]
pub struct NetworkEventWithIdIdCounter(pub u64);
#[derive(Debug)]
pub struct SentActionInfo {
    pub action_payload: NetworkEvent, // Store for potential retry
    pub time_sent: Duration,
    pub retries: u8,
}

#[derive(Resource, Default, Debug)]
pub struct PendingServerAcks(pub HashMap<u64, SentActionInfo>);
#[derive(Component)]
#[require(PlayerState)]
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
pub struct LocalPlayer {
    pub id: u64,
}
#[derive(Component, Default)]
pub struct MeshEntityLink(pub Option<Entity>);
#[derive(Component, Default, Resource)]
pub struct LocalPlayerMesh;
#[derive(Resource)]
pub struct ServerIP(pub String);
#[derive(Resource, Default)]
pub struct PlayerInput {
    pub move_direction: Vec2,
    pub jump: bool,
    // Add other input states as needed
}
#[derive(Copy, Component, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum Direction {
    Forward,
    Left,
    Right,
    Backward,
}

#[derive(Component)]
pub struct SceneName {
    pub scene_name: String,
    pub id: u64,
    pub parent: Entity,
}
#[derive(Component)]
pub struct AnimateableEntity;
#[derive(Component)]
pub struct KnockdownEventAdded;
#[derive(Resource)]
pub struct EsmReaderResource(pub EsmReader<'static>);

#[derive(Resource)]
pub struct AllLandscapes(pub HashMap<(i32, i32), Landscape>);
#[derive(Resource)]
pub struct AllLandTextures(pub HashMap<u32, LandTexture>);
#[derive(Component)]
pub struct LandscapeAwaitingImages {
    pub image_handles: Vec<Handle<Image>>,
    pub texture_index_map: HashMap<u16, usize>,
}
#[derive(Component)]
pub struct LandscapeLoader {
    pub cell_x: i32,
    pub cell_y: i32,
}

pub const MORROWIND_CELL_SIZE: f32 = 40.96;
pub const MORROWIND_HEIGHT_SCALE: f32 = 0.04;
pub const COLLIDER_HEIGHT: f32 = 1.2;
pub const MESH_HEIGHT_OFFSET: f32 = 0.9;
#[derive(Resource, Default)]
pub struct IdToEntityMap(pub HashMap<u64, Entity>);
fn add_creature_to_id_map(
    mut world: DeferredWorld,
    HookContext {
        entity: creature_entity,
        ..
    }: HookContext,
) {
    let id_to_insert_option = world
        .get::<CreatureStats>(creature_entity)
        .map(|stats| stats.id);

    if let Some(actual_id) = id_to_insert_option {
        world
            .resource_mut::<IdToEntityMap>()
            .0
            .insert(actual_id, creature_entity);
    } else {
        panic!(
            "CreatureStats not found on entity {:?} for IdToEntityMap insertion",
            creature_entity
        );
    }
}
