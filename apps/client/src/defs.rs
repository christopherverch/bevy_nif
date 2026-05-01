use crate::animation::AnimationConfig;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use bevy_nif::nif_animation::NifEventType;
use common::EntityId;
use esm_parser_lib::{EsmReader, LandTexture, Landscape};
use networking::NetworkEvent;
use serde::{Deserialize, Serialize};
use serializer::SerializableTransform;
use std::collections::HashMap;
use std::time::{Duration, Instant};

// We want this to be its own component so we can add it to actors, creatures, etc
#[derive(Component, Clone, Debug, Serialize, Deserialize, Resource)]
#[component(on_add = add_creature_to_id_map)]
pub struct CreatureStats {
    pub entity_id: EntityId,
    pub health: f32,
    pub nif_animator_link: Option<Entity>,
}
#[derive(Component, Debug)]
pub struct FirstPersonMeshEntityLinks(pub Vec<Entity>);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum NetworkAnimEvent {
    NifAnimEvent {
        event_type: NifEventType,
        skeleton_id: EntityId,
    },
    BevyAnimEvent {
        animation_config: AnimationConfig,
        skeleton_id: EntityId,
    },
}

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
pub struct NetworkedEntity {
    pub entity_id: EntityId,
    pub entity: Entity,
    pub serializable_transform: SerializableTransform,
    pub last_packet_time: Instant,
}
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
pub struct Player;
#[derive(Debug, Resource)]
pub struct ClientEntityId(pub u64);

#[derive(Component, Resource, Default)]
pub struct IsThirdPerson(pub bool);
#[derive(Component)]
pub struct NeedsSetup {
    pub entity_id: EntityId,
    pub player_transform: Transform,
}
#[derive(Component)]
pub struct PlayerCamera;
#[derive(Component, Default)]
pub struct LocalPlayer {
    pub entity_id: EntityId,
}
#[derive(Component)]
pub struct MeshEntityLink(pub Entity);
#[derive(Component)]
pub struct ColliderEntityLink(pub Entity);
#[derive(Component, Default)]
pub struct LocalPlayerMesh;
#[derive(Resource)]
pub struct ServerIP(pub String);
#[derive(Resource, Default, Debug)]
pub struct PlayerInput {
    pub move_direction: Vec2,
    pub rotation_z: f32, // Rotation around the Z axis this frame
    pub attack_is_held: bool,
    pub attack_just_pressed: bool,
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
pub struct AnimateableEntity;
#[derive(Resource)]
pub struct EsmReaderResource(pub EsmReader);

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

// add this creature's id to the id->Entity hashmap, so we can get the entity by its id
fn add_creature_to_id_map(
    mut world: DeferredWorld,
    HookContext {
        entity: creature_entity,
        ..
    }: HookContext,
) {
    let entity_id_opt = world
        .get::<CreatureStats>(creature_entity)
        .map(|stats| stats.entity_id);

    if let Some(actual_id) = entity_id_opt {
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
