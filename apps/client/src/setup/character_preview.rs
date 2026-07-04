use bevy::prelude::*;
use bevy_nif::{
    attach_parts::AttachmentType,
    hash_str,
    spawner::{NifInstantiated, NifNodeIndex, NifScene},
};

pub fn spawn_base_anim_skeleton(
    asset_server: &Res<AssetServer>,
    commands: &mut Commands,
    transform: Transform,
    skeleton_id: u64,
) -> Entity {
    let asset_handle = asset_server.load("data/base_anim.nif");
    commands
        .spawn((
            NifScene(asset_handle),
            AttachmentType::MainSkeleton { skeleton_id },
            Visibility::Inherited,
            transform,
        ))
        .id()
}
pub struct RaceSelectPreviewEntities {
    pub skeleton_entity: Entity,
    pub head_entity: Entity,
    pub hair_entity: Entity,
}
pub fn spawn_race_select_preview<S1, S2, M1, M2>(
    parent_entity: Entity,
    transform: Transform,
    skeleton_id: u64,
    hair_path: &str,
    head_path: &str,
    asset_server: &Res<AssetServer>,
    commands: &mut Commands,
    skeleton_loaded_fn: S1,
    head_hair_loaded_fn: S2,
) -> RaceSelectPreviewEntities
where
    S1: IntoSystem<In<(Entity, Option<Entity>)>, (), M1> + Send + Sync + Clone + 'static,
    S2: IntoSystem<(), (), M2> + Send + Sync + Clone + 'static,
    M1: 'static,
    M2: 'static,
{
    let skeleton_entity = spawn_base_anim_skeleton(asset_server, commands, transform, skeleton_id);
    commands.entity(skeleton_entity).observe(
        move |event: On<NifInstantiated>,
              nif_index_q: Query<&NifNodeIndex>,
              mut commands: Commands| {
            let Ok(nif_index) = nif_index_q.get(event.entity) else {
                error!("Race preview skeleton was missing nif node index!");
                return;
            };
            let head_entity_opt = get_bone_entity(nif_index, "Bip01 Head");
            let skeleton_fn = skeleton_loaded_fn.clone();
            commands.run_system_cached_with(skeleton_fn, (event.entity, head_entity_opt));
        },
    );
    commands.entity(parent_entity).add_child(skeleton_entity);
    let paths_with_target_bone: Vec<(&str, AttachmentType)> = vec![
        (
            head_path,
            AttachmentType::Rigid {
                target_bone: "Head".to_string(),
                skeleton_id,
            },
        ),
        (
            hair_path,
            AttachmentType::Rigid {
                target_bone: "Head".to_string(),
                skeleton_id,
            },
        ),
    ];
    let mut head_entity = Entity::PLACEHOLDER;
    let mut hair_entity = Entity::PLACEHOLDER;

    let mut is_hair = false;
    for (path, attachment_type) in paths_with_target_bone {
        let head_hair_fn = head_hair_loaded_fn.clone();
        let asset_handle = asset_server.load(path.to_string());
        let child = commands
            .spawn((
                NifScene(asset_handle),
                attachment_type,
                Transform::default(),
                Visibility::Inherited,
            ))
            .id();
        commands.entity(child).observe(
            move |_event: On<NifInstantiated>, mut commands: Commands| {
                let head_hair_fn = head_hair_fn.clone();
                commands.run_system_cached(head_hair_fn);
            },
        );
        if !is_hair {
            head_entity = child;
        } else {
            hair_entity = child;
        }
        is_hair = true;
        commands.entity(skeleton_entity).add_child(child);
    }
    RaceSelectPreviewEntities {
        skeleton_entity,
        head_entity,
        hair_entity,
    }
}
pub fn get_bone_entity(nif_index: &NifNodeIndex, name: &str) -> Option<Entity> {
    let hash = hash_str(name);
    nif_index.named_nodes.get(&hash).copied()
}
