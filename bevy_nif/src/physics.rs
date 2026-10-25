use avian3d::prelude::Collider;
use bevy_ecs::{
    lifecycle::Add,
    observer::On,
    system::{Commands, Query},
};

use crate::loader_spawners::TempNifCollider;

pub fn replace_temp_colliders(
    ev: On<Add<TempNifCollider>>,
    mut temp_nif_collider_q: Query<&mut TempNifCollider>,
    mut commands: Commands,
) {
    let mut temp_nif_collider = temp_nif_collider_q.get_mut(ev.entity).unwrap();
    commands.entity(ev.entity).insert(
        Collider::try_trimesh(
            std::mem::take(&mut temp_nif_collider.0.0),
            std::mem::take(&mut temp_nif_collider.0.1),
        )
        .unwrap(),
    );
}
