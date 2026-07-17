use bevy::prelude::*;
use bevy_nif::{
    attach_parts::AttachmentType,
    hash_str,
    loader::Nif,
    spawner::{NifInstantiated, NifNodeIndex, NifScene},
};
#[derive(Component)]
pub struct PreviewHeadBoneEntity;

pub fn spawn_base_anim_skeleton(
    nif_skeleton_handle: Handle<Nif>,
    commands: &mut Commands,
    transform: Transform,
    skeleton_id: u64,
) -> Entity {
    commands
        .spawn((
            NifScene(nif_skeleton_handle),
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
pub fn spawn_race_select_preview(
    nif_skeleton_handle: Handle<Nif>,
    parent_entity: Entity,
    transform: Transform,
    skeleton_id: u64,
    hair_path: &str,
    head_path: &str,
    asset_server: &Res<AssetServer>,
    commands: &mut Commands,
) -> RaceSelectPreviewEntities
where
{
    let skeleton_entity =
        spawn_base_anim_skeleton(nif_skeleton_handle, commands, transform, skeleton_id);
    commands.entity(skeleton_entity).observe(
        move |event: On<NifInstantiated>,
              nif_index_q: Query<&NifNodeIndex>,
              mut commands: Commands| {
            let Ok(nif_index) = nif_index_q.get(event.entity) else {
                error!("Race preview skeleton was missing nif node index!");
                return;
            };
            let head_entity_opt = get_bone_entity(nif_index, "Bip01 Head");
            if let Some(head_bone_entity) = head_entity_opt {
                commands
                    .entity(head_bone_entity)
                    .insert(PreviewHeadBoneEntity);
            }
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
        let asset_handle = asset_server.load(path.to_string());
        let child = commands
            .spawn((
                NifScene(asset_handle),
                attachment_type,
                Transform::default(),
                Visibility::Inherited,
            ))
            .id();
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
