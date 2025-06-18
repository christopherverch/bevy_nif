use bevy::color::palettes::basic;
use bevy::prelude::*;
use bevy_rapier3d::{plugin::ReadRapierContext, prelude::QueryFilter, render::ColliderDebugColor};
use cdda_bevy::client::{
    defs::{AttackState, CreatureStats, PlayerState},
    events::{TriggerHit, TryAttackTrigger},
};
use std::time::Instant;

use crate::{const_defs::MESH_HEIGHT_OFFSET, utils::create_line_mesh};

pub fn execute_attack(
    attacker_entity: Entity,
    player_state: &mut PlayerState,
    camera_transform: &Transform, // To get attack direction
    commands: &mut Commands,
) {
    let forward_vector = camera_transform.forward();
    let ray_direction = forward_vector.normalize();
    // Ray origin from player's current transform
    let ray_origin =
        player_state.transform.translation + Vec3::new(0.0, MESH_HEIGHT_OFFSET + 0.3, 0.0);

    player_state.attack_state = AttackState::Releasing;
    let charge_duration_ms = player_state.started_attacking.elapsed().as_millis();
    // Calculate damage based on the charge duration
    let damage = 5.0 + 10.0 * (charge_duration_ms as f32 / 1200.0).min(1.0);

    commands.trigger(TryAttackTrigger {
        attacker_entity,
        attacker_id: player_state.id, // Make sure PlayerState has `id`
        origin: ray_origin,
        direction: ray_direction,
        damage,
    });

    // Reset the attack timer AFTER using its elapsed value for damage calculation
    player_state.started_attacking = Instant::now();
}
pub fn trigger_attack(
    trigger: Trigger<TryAttackTrigger>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    rapier_context: ReadRapierContext,
    creature_q: Query<&CreatureStats>,
) {
    println!(
        "drawing ray from: {} to {}",
        trigger.origin, trigger.direction
    );
    let mesh = create_line_mesh(trigger.origin, trigger.origin + (trigger.direction * 2.0));
    let sphere_color = Color::srgb(10.0, 4.0, 1.0);
    // Emissive sphere
    let unlit_mat: StandardMaterial = sphere_color.into();
    let material_h = materials.add(unlit_mat);
    let mesh_handle = meshes.add(mesh);
    commands.spawn((Mesh3d(mesh_handle), MeshMaterial3d(material_h)));
    let Ok(context) = rapier_context.single() else {
        return;
    };

    println!("ray_direction: {}", trigger.direction);
    let hit = context.cast_ray(
        trigger.origin,
        trigger.direction.into(),
        2.0,
        true,
        QueryFilter {
            exclude_collider: Some(trigger.attacker_entity),
            ..default()
        },
    );
    if let Some((entity, _toi)) = hit {
        println!("hit entity: {}", entity);
        if let Ok(creature) = creature_q.get(entity) {
            commands.trigger(TriggerHit {
                attacker_id: trigger.attacker_id,
                receiver_id: creature.id,
                damage: trigger.damage,
                networked: true,
            });
        }
        // Color in blue the entity we just hit.
        // Because of the query filter, only colliders attached to a dynamic body
        // will get an event.
        let color = basic::BLUE.into();
        commands.entity(entity).insert(ColliderDebugColor(color));
    }
}
