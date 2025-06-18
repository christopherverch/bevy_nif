use crate::serializer::PlayerStateNetwork;
use crate::{AnimationConfig, NetworkEvent};
use bevy::prelude::*;
use bevy_ecs::component::HookContext;
use bevy_ecs::world::DeferredWorld;
use bevy_nif::nif_animation::NifEventType;
use esm_parser_lib::{EsmReader, LandTexture, Landscape};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Resource, Component, Debug)]
pub struct PlayerState {
    pub id: u64,
    pub transform: Transform,
    pub last_packet_time: Instant,
    pub started_attacking: Instant,
    pub is_attacking: bool,
    pub is_ready_weapon: bool,
    pub attack_state: AttackState,
}
impl Default for PlayerState {
    fn default() -> Self {
        PlayerState {
            id: 0,
            transform: Transform::default(),
            last_packet_time: Instant::now(),
            started_attacking: Instant::now(),
            is_attacking: false,
            is_ready_weapon: false,
            attack_state: AttackState::Inactive,
        }
    }
}
impl PlayerState {
    pub fn update_from_network(&mut self, net: PlayerStateNetwork) {
        self.transform = net.transform.into();
        self.attack_state = net.attack_state;
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)] // Add Hash if used as a HashMap key
pub enum LocomotionState {
    Idle,
    Moving(Direction), // e.g., Moving(Direction::Forward) for runforward
    Jumping,
    Falling,
    Landing,
    FinishedLanding,
    Knockdown,
}

//we want this to be its own component so we can add it to actors, creatures, etc
#[derive(Component, Clone, Debug, Serialize, Deserialize, Resource)]
#[component(on_add = add_creature_to_id_map)]
pub struct CreatureStats {
    pub id: u64,
    pub health: f32,
    pub nif_animator_link: Option<Entity>,
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
#[derive(Component, Debug)]
pub struct FirstPersonMeshEntityLinks(pub Vec<Entity>);
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum NetworkAnimEvent {
    NifAnimEvent {
        event_type: NifEventType,
        skeleton_id: u64,
    },
    BevyAnimEvent {
        animation_config: AnimationConfig,
        skeleton_id: u64,
    },
}

#[derive(Component, Debug)]
pub struct NifAnimatorLink(pub Entity);
#[derive(Component)]
pub struct Animator {
    pub graph: Handle<AnimationGraph>,
    pub animations: Vec<AnimationNodeIndex>,
}

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
pub struct RootEntityLink(pub Option<Entity>);
#[derive(Component)]
pub struct NetworkedEntity(pub Entity);
#[derive(Resource, Default, Debug)]
pub struct NetworkEventWithIdCounter(pub u64);
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

#[derive(Component, Resource, Default)]
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
#[derive(Resource, Default, Debug)]
pub struct PlayerInput {
    pub move_direction: Vec2,
    pub rotation_z: f32, // Rotation around the Z axis this frame
    pub jump: bool,
    pub run: bool,
    pub sneak: bool,
}
#[derive(Copy, Component, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Left,
    Right,
    Backward,
    None,
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
pub struct LandTextureDefinitions(pub HashMap<u32, LandTexture>);
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
#[derive(Resource, Debug, Copy, Clone)]
pub struct LandscapeManagementSettings {
    pub load_radius_cells: u32, // e.g., 2 means a 5x5 grid (current + 2 in each direction)
    pub unload_radius_cells: u32, // e.g., 3 (larger than load_radius to prevent rapid load/unload)
}
#[derive(Resource, Default, Debug)]
pub struct LoadedLandscapeEntities {
    // Maps cell (x, y) coordinates to the Bevy Entity ID that represents that landscape cell's mesh.
    pub cell_to_entity: HashMap<(i32, i32), Entity>,
}
#[derive(Component, Debug, Copy, Clone)]
pub struct LandscapeCell {
    pub x: i32,
    pub y: i32,
}
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
