use crate::magic_effects::{EffectKey, MagicEffects};
use crate::stat::{AttributeValue, DynamicStat};
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use bitflags::bitflags;
use common::EntityId;
use esm_parser_lib::types::attribute::Attribute;
use esm_parser_lib::types::enums::AttributeId;
use esm_parser_lib::types::gmst::GameSetting;
use esm_parser_lib::types::magic_effect::MagicEffect;
use esm_parser_lib::{EsmReader, LandTexture, RefId};
use esm_store::{EsmStore, Store};
use networking::NetworkEvent;
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use serializer::SerializableTransform;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};
#[derive(Component)]
pub struct Movement {
    /// Relative to the current orientation
    /// Length of the vector controls desired speed. 0 - stay, 0.5 - half-speed, 1.0 - max speed.
    pub desired_movement: Vec3,
    /// rotation delta (euler) pitch yaw roll
    pub desired_rotation: Vec3,
    /// internal and should not be changed
    pub speed_factor: f32,
    /// internal and should not be changed
    pub is_strafing: bool,
}
impl Default for Movement {
    fn default() -> Self {
        Self {
            desired_movement: Vec3::ZERO,
            desired_rotation: Vec3::ZERO,
            speed_factor: 1.0,
            is_strafing: false,
        }
    }
}
pub enum Stance {
    Run,
    Sneak,
}
#[derive(Clone, Debug, PartialEq)]
pub enum DrawState {
    Nothing,
    Weapon,
    Spell,
}

#[derive(Resource)]
pub struct WorldSettings {
    pub god_mode: bool,
    pub levitation_enabled: bool,
    pub swim_height_scale: f32,
    pub rng: StdRng,
}
impl Default for WorldSettings {
    fn default() -> Self {
        Self {
            god_mode: Default::default(),
            levitation_enabled: Default::default(),
            swim_height_scale: Default::default(),
            rng: {
                let rng = [0; 32];
                StdRng::from_seed(rng)
            },
        }
    }
}
#[derive(Resource)]
pub struct GameSettings {
    pub normalize_race_speed: bool,
    pub smooth_movement: bool,
    pub smooth_movement_player_turning_delay: f32,
    pub turn_to_movement_direction: bool,
    pub player_movement_ignores_animation: bool,
}
impl Default for GameSettings {
    fn default() -> Self {
        Self {
            normalize_race_speed: Default::default(),
            smooth_movement: Default::default(),
            smooth_movement_player_turning_delay: 1.0,
            turn_to_movement_direction: false,
            player_movement_ignores_animation: false,
        }
    }
}
#[derive(Resource, Default)]
pub struct WorldFrameData {
    pub is_in_storm: bool,
    pub storm_direction: Vec3,
}
#[derive(Component)]
pub struct MWCell {
    pub has_water: bool,
    pub water_level: f32,
}
#[derive(Component)]
pub struct CurrentCell {
    pub coords: UVec2,
}
#[derive(Resource, Default)]
pub struct CellLookup(pub HashMap<UVec2, Entity>);

#[derive(Resource)]
pub struct Pause(pub bool);
#[derive(Component)]
pub struct TestingComponent;
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
pub struct Player {
    pub jumping: bool,
}
#[derive(Debug, Resource)]
pub struct ClientEntityId(pub u64);

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
#[derive(Resource, Default, Debug)]
pub struct PlayerInput {
    pub rotation_z: f32, // Rotation around the Z axis this frame
    pub attack_is_held: bool,
    pub attack_just_pressed: bool,
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

pub static ESM_READER: OnceLock<RwLock<EsmReader>> = OnceLock::new();
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
