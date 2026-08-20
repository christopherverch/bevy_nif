use avian3d::prelude::LayerMask;
use bevy::prelude::*;

use crate::physics::{
    collision::{CollisionShapeType, MWCollisionGroup},
    position_interpolate::PositionInterpolation,
};

#[derive(Component)]
pub struct ActorPhysics {
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
impl ActorPhysics {
    pub fn new(
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
        let half_extents = collision_shape_extents / 2.0;

        let rendering_half_extents = Vec3::ZERO;
        ActorPhysics {
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
    pub fn is_on_ground(&self) -> bool {
        self.on_ground && self.internal_collision_mode
    }
    pub fn set_can_water_walk(
        &mut self,
        can_water_walk: bool,
        collision_layer_filters: &LayerMask,
    ) -> u32 {
        if can_water_walk != self.can_water_walk {
            self.can_water_walk = can_water_walk;
            self.update_collision_mask()
        } else {
            collision_layer_filters.0
        }
    }
    pub fn update_collision_mask(&self) -> u32 {
        self.get_collision_mask().bits()
    }
    pub fn get_collision_object_position(&self, current_translation: Vec3) -> Vec3 {
        self.get_scaled_mesh_translation() + current_translation
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
    pub fn set_position(
        &mut self,
        position_interpolation: &mut PositionInterpolation,
        position: Vec3,
    ) -> bool {
        let world_position_changed = self.position_offset.length_squared() != 0.0;
        self.apply_offset_change(position_interpolation);
        // Position offsets override any actual physics that happened this frame, so we
        // don't set any positions if there was an offset
        if world_position_changed {
            return true;
        }
        position_interpolation.previous_position = position_interpolation.current_position;
        position_interpolation.current_position = position;
        // true if this new position is different, false if it's the same
        return position_interpolation.previous_position != position_interpolation.current_position;
    }
    pub fn apply_offset_change(&mut self, position_interpolation: &mut PositionInterpolation) {
        if self.position_offset.length_squared() != 0.0 {
            position_interpolation.current_position += self.position_offset;
            position_interpolation.previous_position += self.position_offset;
            self.position_offset = Vec3::ZERO;
        }
    }
}

/// Scratch board for the physics timestep, so the order the entities are processed doesn't matter
/// since they use the other actor's ActorPhysics struct, and this is only applied at the end of the frame.
#[derive(Component)]
pub struct ActorFrameData {
    // Basic state
    pub position: Vec3,
    pub standing_on: Option<Entity>,
    pub is_on_ground: bool,
    pub is_on_slope: bool,
    pub walking_on_water: bool,
    pub inert: bool,

    // Swimming logic
    pub swim_level: f32, // water_level - (rendering_half_extents_z * 2 * swim_height_scale)
    pub slow_fall_mult: f32,
    pub water_level: f32,

    // Transform/Movement
    pub rotation: Vec2,
    pub movement: Vec3,
    pub half_extents_y: f32,
    pub inertia: Vec3,
    pub old_height: f32,
    pub last_stuck_position: Vec3,
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
        actor: &ActorPhysics,
        inert: bool,
        water_collision: bool,
        slow_fall_mult: f32,
        water_level: f32,
        is_player: bool,
        swim_height_scale: f32,
        flying: bool,
        is_aquatic: bool,
        velocity: Vec3,
    ) -> Self {
        let swim_level = water_level - (actor.rendering_half_extents.y * 2.0 * swim_height_scale);

        Self {
            position: Vec3::ZERO,
            standing_on: None,
            is_on_ground: actor.on_ground,
            is_on_slope: actor.on_slope,
            walking_on_water: false,
            inert,
            swim_level,
            slow_fall_mult,
            rotation: Vec2::ZERO,
            movement: velocity,
            water_level,
            half_extents_y: actor.half_extents.y,
            inertia: Vec3::ZERO,
            old_height: 0.0,
            last_stuck_position: Vec3::ZERO,
            stuck_frames: 0,

            flying: flying,
            was_on_ground: actor.on_ground,
            is_aquatic,
            water_collision,
            skip_collision_detection: !actor.internal_collision_mode,
            is_player,
        }
    }
}
