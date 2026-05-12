use crate::{
    collision::{CollisionShapeType, MWCollisionGroup},
    ptr_holder::PtrHolder,
};
use bevy_ecs::{component::Component, entity::Entity};
use glam::{DVec3, Quat, Vec2, Vec3};
#[derive(Component)]
pub struct Actor {
    pub base: PtrHolder,

    // Movement Logic
    pub standing_on: Option<Entity>,
    pub force: Vec3,
    pub on_ground: bool,
    pub on_slope: bool,
    pub stuck_frames: u32,
    pub last_stuck_position: Vec3,
    pub position_offset: Vec3,

    // Water Logic
    pub can_water_walk: bool,
    pub walking_on_water: bool,

    // Shape/Scale Metadata
    pub collision_shape_type: CollisionShapeType,
    pub rotationally_invariant: bool,
    pub mesh_translation: Vec3,
    pub original_half_extents: Vec3,
    pub half_extents: Vec3,           // Logic uses this for collisions
    pub rendering_half_extents: Vec3, // Rendering uses this for bounds
    pub scale: Vec3,
    pub rotation: Quat,

    // Mode Flags
    pub internal_collision_mode: bool,
    pub external_collision_mode: bool,
    pub skip_simulation: bool,
    pub active: bool,
}
impl Actor {
    pub fn new(
        base: PtrHolder,
        can_water_walk: bool,
        on_ground: bool,
        collision_shape_type: CollisionShapeType,
        rotationally_invariant: bool,
        collision_shape_center: Vec3,
        collision_shape_extents: Vec3,
    ) -> Self {
        //TODO::
        let scale = Vec3::ONE;
        let rotation = Quat::default();
        let position_offset = Vec3::ZERO;
        let half_extents = Vec3::ONE;

        let rendering_half_extents = Vec3::ZERO;
        Actor {
            base,
            standing_on: None,
            force: Vec3::ZERO,
            on_ground,
            on_slope: false,
            stuck_frames: 0,
            last_stuck_position: Vec3::ZERO,
            position_offset,
            rotation,
            can_water_walk,
            walking_on_water: false,
            collision_shape_type,
            rotationally_invariant,
            mesh_translation: collision_shape_center,
            original_half_extents: collision_shape_extents,
            half_extents,
            rendering_half_extents,
            scale,
            internal_collision_mode: true,
            external_collision_mode: true,
            skip_simulation: true,
            active: false,
        }
    }
    pub fn get_collision_object_position(&self) -> Vec3 {
        self.get_scaled_mesh_translation() + self.base.position.as_vec3()
    }
    /// Moves the actor by a specific offset (usually from scripts) and resets the offset.
    pub fn apply_offset_change(&mut self) -> DVec3 {
        if self.position_offset.length_squared() > 1e-6 {
            let offset_d = self.position_offset.as_dvec3();
            self.base.position += offset_d;
            self.base.previous_position += offset_d;
            self.base.simulation_position += self.position_offset;
            self.position_offset = Vec3::ZERO;
        }
        self.base.position
    }

    pub fn update_scale_unsafe(&mut self, physics_scale: Vec3, rendering_scale: Vec3) {
        self.scale = physics_scale;
        self.half_extents = self.original_half_extents * physics_scale;
        self.rendering_half_extents = self.original_half_extents * rendering_scale;
    }

    pub fn get_collision_mask(&self) -> MWCollisionGroup {
        let mut mask = MWCollisionGroup::WORLD | MWCollisionGroup::HEIGHTMAP;

        if self.external_collision_mode {
            mask |= MWCollisionGroup::ACTOR | MWCollisionGroup::PROJECTILE | MWCollisionGroup::DOOR;
        }

        if self.can_water_walk {
            mask |= MWCollisionGroup::WATER;
        }

        mask
    }
    pub fn get_scaled_mesh_translation(&self) -> Vec3 {
        // rotation * (translation * scale)
        self.rotation * (self.mesh_translation * self.scale)
    }
}

/// Scratch board for the physics timestep, so the order the entities are processed doesn't matter
/// since they use the other actor's Actor struct, and this is only applied at the end of the frame.
pub struct ActorFrameData {
    // Basic state
    pub position: Vec3,
    pub standing_on: Option<Entity>,
    pub is_on_ground: bool,
    pub is_on_slope: bool,
    pub walking_on_water: bool,
    pub inert: bool,

    // Physics body reference
    // In Bevy/Avian, this might just be the Entity with the Collider
    pub collision_object: Entity,

    // Swimming logic
    pub swim_level: f32, // water_level - (rendering_half_extents_z * 2 * swim_height_scale)
    pub slow_fall: f32,
    pub water_level: f32,

    // Transform/Movement
    pub rotation: Vec2,
    pub movement: Vec3,
    pub half_extents_y: f32,
    pub old_height: f32,
    pub stuck_frames: u32,

    // Flags
    pub flying: bool,
    pub was_on_ground: bool,
    pub is_aquatic: bool,
    pub water_collision: bool,
    pub skip_collision_detection: bool,
    pub is_player: bool,
}

impl ActorFrameData {
    pub fn new(
        actor: &Actor,
        inert: bool,
        water_collision: bool,
        slow_fall: f32,
        water_level: f32,
        is_player: bool,
    ) -> Self {
        // TODO:: MWBase::Environment::get()
        // .getESMStore()
        //->get<ESM::GameSetting>()
        // .find("fSwimHeightScale")
        //->mValue.getFloat()))
        let swim_height_scale = 1.0;
        let swim_level = water_level - (actor.rendering_half_extents.y * 2.0 * swim_height_scale);

        Self {
            position: Vec3::ZERO,
            standing_on: None,
            is_on_ground: actor.on_ground,
            is_on_slope: actor.on_slope,
            walking_on_water: false,
            inert,
            collision_object: actor.base.entity,
            swim_level,
            slow_fall,
            rotation: Vec2::ZERO,
            movement: actor.base.velocity,
            water_level,
            half_extents_y: actor.half_extents.y,
            old_height: 0.0,
            stuck_frames: 0,

            // TODO:: mFlying(MWBase::Environment::get().getWorld()->isFlying(actor.getPtr()))
            flying: false,
            // TODO::
            //mIsAquatic(actor.getPtr().getClass().isPureWaterCreature(actor.getPtr()))
            was_on_ground: actor.on_ground,
            is_aquatic: false,
            water_collision,
            skip_collision_detection: !actor.internal_collision_mode,
            is_player,
        }
    }
}
