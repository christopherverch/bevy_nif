use crate::{
    animation::NifAnimatorLink,
    camera::MWCamera,
    defs::{IdToEntityMap, LocalPlayer, MeshEntityLink},
    mechanics::creature_stats::CreatureStats,
};
use bevy::prelude::*;
use bevy_nif::nif_animation::NifAnimator;
use common::consts::MW_UNITS_PER_METER;
use common::mechanics::{CharacterController, JumpingStateKind};

use crate::animation::animation_helpers::get_anim_components_for_creature;
/// A component that stores the calculated root motion displacement for a single frame.
/// This is calculated by one system and consumed by the physics movement system.
#[derive(Component, Debug, Default)]
pub struct RootMotionState {
    /// The name of the animation group that was playing last frame.
    pub last_animation_group: String,
    /// The elapsed duration of the animation from the previous frame.
    pub previous_time: f32,
}
// Speed determined by how far the movement animation moved in this frame
// ex: runforward moved 0.1 meter forward since last frame, this would be 0.1
// used for the physics movement system to determine how far we should move in 3rd person
#[derive(Component, Default)]
pub struct AnimationDisplacement(pub Vec3);

pub fn calculate_animation_displacement_system(
    mut creature_mesh_query: Query<&mut RootMotionState>,
    actor_q: Query<(
        &CharacterController,
        &CreatureStats,
        &NifAnimatorLink,
        &mut AnimationDisplacement,
        &MeshEntityLink,
        Option<&LocalPlayer>,
    )>,
    mut animator_q: Query<(
        &mut AnimationPlayer,
        &mut NifAnimator,
        &AnimationGraphHandle,
    )>,
    cam_settings: Single<&MWCamera>,
    id_to_entity_map: Res<IdToEntityMap>,
) {
    for (
        actor_state,
        creature_stats,
        creature_animator_link,
        mut animation_displacement,
        mesh_entity_link,
        local_player_opt,
    ) in actor_q
    {
        // This system only matters in third-person view.
        if cam_settings.first_person && local_player_opt.is_some() {
            // In first-person view, root motion is not applied. We explicitly clear
            // any existing animation displacement to prevent it from affecting the camera.
            animation_displacement.0 = Vec3::ZERO;
            return;
        }

        // Get the root motion state of the mesh
        let Ok(mut root_motion_state) = creature_mesh_query.get_mut(mesh_entity_link.0) else {
            return;
        };

        // Root motion is ignored while jumping/falling. The speed is zero.
        if actor_state.jumping.state != JumpingStateKind::None {
            animation_displacement.0 = Vec3::ZERO;
            return;
        }

        let Some((_, animation_player, animator, _)) = get_anim_components_for_creature(
            creature_stats.entity_id,
            creature_animator_link.0,
            &id_to_entity_map,
            &mut animator_q,
        ) else {
            return;
        };

        let mut frame_displacement = Vec3::ZERO;
        let current_move_group = &actor_state.movement.current_group;

        if !current_move_group.is_empty() {
            if let Some(definition) = animator.animation_definitions.get(current_move_group) {
                if let Some(curve) = &definition.root_translation_curve {
                    if let Some(anim_state) = animation_player.animation(definition.node_index) {
                        let current_time = anim_state.elapsed() % definition.duration;

                        // if we have a new move group or the elapsed time is less than the stored time
                        if current_move_group != &root_motion_state.last_animation_group
                            || current_time < root_motion_state.previous_time
                        {
                            // Act as if this animation just started
                            root_motion_state.previous_time = current_time;
                        }
                        // Calculate the displacement delta since the last frame, handling animation loops correctly.
                        let delta = if current_time < root_motion_state.previous_time {
                            // The animation has looped since the last frame. To get the correct
                            // displacement, we calculate it in two parts:

                            // 1. The displacement from the previous frame's timestamp to the end of the animation.
                            let pos_at_end_opt = curve.sample(definition.duration);
                            let pos_at_prev_opt = curve.sample(root_motion_state.previous_time);
                            let delta1 = pos_at_end_opt
                                .zip(pos_at_prev_opt)
                                .map_or(Vec3::ZERO, |(end, prev)| end - prev);

                            // 2. The displacement from the beginning of the animation to the new, smaller timestamp.
                            let pos_at_start_opt = curve.sample(0.0);
                            let pos_at_now_opt = curve.sample(current_time);
                            let delta2 = pos_at_start_opt
                                .zip(pos_at_now_opt)
                                .map_or(Vec3::ZERO, |(start, now)| now - start);

                            delta1 + delta2
                        } else {
                            // Normal frame, no loop.
                            let pos_now_opt = curve.sample(current_time);
                            let pos_prev_opt = curve.sample(root_motion_state.previous_time);
                            pos_now_opt
                                .zip(pos_prev_opt)
                                .map_or(Vec3::ZERO, |(now, prev)| now - prev)
                        };

                        let scaled_delta = (delta / MW_UNITS_PER_METER) * anim_state.speed();
                        // Transform from NIF's coordinate system (Y-up, X-right) to Bevy's (Y-up, -Z-forward).
                        // The vertical (Y) component of root motion is discarded
                        // since the physics system handles y-motion with gravity
                        // TODO:: check if openmw flattens the y (scaled_delta.z) to 0.0
                        frame_displacement = Vec3::new(scaled_delta.x, 0.0, -scaled_delta.y);

                        root_motion_state.previous_time = current_time;
                        root_motion_state.last_animation_group = current_move_group.clone();
                    }
                }
            }
        }
        // The final calculated displacement for this frame is stored in the AnimationDisplacement component.
        // This value is consumed by `prepare_character_velocity_system`, which converts it into a
        // velocity before passing it to the movement solver.
        animation_displacement.0 = frame_displacement;
    }
}
