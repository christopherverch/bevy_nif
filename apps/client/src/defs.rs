use crate::animation::AnimationConfig;
use crate::magic_effects::MagicEffects;
use crate::stat::DynamicStat;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use bevy_nif::nif_animation::NifEventType;
use bitflags::bitflags;
use common::EntityId;
use esm_parser_lib::{EsmReader, LandTexture, Landscape};
use networking::NetworkEvent;
use serde::{Deserialize, Serialize};
use serializer::SerializableTransform;
use std::collections::HashMap;
use std::time::{Duration, Instant};
#[derive(Component)]
pub struct Npc;
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
bitflags! {
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CreatureStatsFlags: u8 {
    const FORCE_RUN       = 1 << 0; // 1
    const FORCE_SNEAK     = 1 << 1; // 2
    const RUN             = 1 << 2; // 4
    const SNEAK           = 1 << 3; // 8
    const FORCE_JUMP      = 1 << 4; // 16
    const FORCE_MOVE_JUMP = 1 << 5; // 32
}}
#[derive(Clone, Debug, PartialEq)]
pub enum DrawState {
    Nothing,
    Weapon,
    Spell,
}
#[derive(Component, Clone, Debug, Resource)]
#[component(on_add = add_creature_to_id_map)]
pub struct CreatureStats {
    pub entity_id: EntityId,
    pub nif_animator_link: Option<Entity>,
    pub magic_effects: MagicEffects,
    pub fall_height: f32,
    // health, magicka, fatigue
    pub dynamic_stats: [DynamicStat<f32>; 3],
    pub dead: bool,
    pub death_animation_finished: bool,
    pub teleported: bool,
    pub movement_flags: CreatureStatsFlags,
    /// Updated every frame before other systems that use it run
    pub max_speed: f32,
    pub side_movement_angle: f32,
    pub draw_state: DrawState,
}
impl CreatureStats {
    pub fn get_health(&self) -> DynamicStat<f32> {
        self.dynamic_stats[0].clone()
    }
    pub fn get_fatigue(&self) -> DynamicStat<f32> {
        self.dynamic_stats[2].clone()
    }
    pub fn set_health(&mut self, value: DynamicStat<f32>) {
        self.set_dynamic(0, value);
    }
    pub fn set_fatigue(&mut self, value: DynamicStat<f32>) {
        self.set_dynamic(2, value);
    }
    fn set_dynamic(&mut self, index: usize, value: DynamicStat<f32>) {
        if index > 2 {
            panic!("dynamic stat index is out of range");
        }
        self.dynamic_stats[index] = value;
        //TODO:: dying
    }
    pub fn land(&mut self, player_opt: Option<&mut Player>) -> f32 {
        if let Some(player) = player_opt {
            player.jumping = false;
        }
        let height = self.fall_height;
        self.fall_height = 0.0;
        return height;
    }
    pub fn is_stance_active(&self, stance: Stance) -> bool {
        match stance {
            Stance::Run => self
                .movement_flags
                .intersects(CreatureStatsFlags::RUN | CreatureStatsFlags::FORCE_RUN),
            Stance::Sneak => self
                .movement_flags
                .intersects(CreatureStatsFlags::SNEAK | CreatureStatsFlags::FORCE_SNEAK),
        }
    }
    pub fn set_movement_flag(&mut self, flag: CreatureStatsFlags, state: bool) {
        if state {
            self.movement_flags |= flag;
        } else {
            self.movement_flags &= !flag;
        }
    }
}

#[derive(Resource, Default)]
pub struct WorldSettings {
    pub god_mode: bool,
    pub levitation_enabled: bool,
    pub third_person: bool,
    pub swim_height_scale: f32,
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
pub struct Cell {
    pub has_water: bool,
    pub water_level: f32,
}
#[derive(Component)]
pub struct CurrentCell {
    pub coords: UVec2,
}
#[derive(Resource, Default)]
pub struct CellLookup(pub HashMap<UVec2, Entity>);
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
#[derive(Resource)]
pub struct ServerIP(pub String);
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
