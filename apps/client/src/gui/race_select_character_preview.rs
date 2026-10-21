use bevy::{diagnostic::FrameCount, prelude::*};
use bevy_nif::{
    attach_parts::AttachmentType,
    hash_str,
    loader::Nif,
    spawner::{NifInstantiated, NifNodeIndex, NifScene},
};

use crate::gui::race_select::{
    RacePreviewScene, RaceSelectPreviewCamera, SliderThumb, set_head_rotation_to_slider,
};
#[derive(EntityEvent)]
#[entity_event(propagate, auto_propagate)]
pub struct PreviewHeadBoneEntityReady(pub Entity);
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
pub fn spawn_race_select_preview(
    nif_skeleton_handle: Handle<Nif>,
    parent_entity: Entity,
    transform: Transform,
    skeleton_id: u64,
    hair_path: &str,
    head_path: &str,
    asset_server: &Res<AssetServer>,
    commands: &mut Commands,
) {
    let skeleton_entity =
        spawn_base_anim_skeleton(nif_skeleton_handle, commands, transform, skeleton_id);
    commands
        .entity(skeleton_entity)
        .observe(on_skeleton_instantiated);
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
        commands.entity(skeleton_entity).add_child(child);
    }
}
pub fn get_bone_entity(nif_index: &NifNodeIndex, name: &str) -> Option<Entity> {
    let hash = hash_str(name);
    nif_index.named_nodes.get(&hash).copied()
}
fn on_skeleton_instantiated(
    event: On<NifInstantiated>,
    nif_index_q: Query<&NifNodeIndex>,
    frame_count: Res<FrameCount>,
    mut slider_thumb: Single<&mut Node, With<SliderThumb>>,
    mut race_preview_scene: Single<
        &mut Transform,
        (With<RacePreviewScene>, Without<RaceSelectPreviewCamera>),
    >,
    global_transforms_q: Query<&GlobalTransform>,
    mut race_preview_camera: Single<
        &mut Transform,
        (With<RaceSelectPreviewCamera>, Without<RacePreviewScene>),
    >,

    mut commands: Commands,
) {
    dbg!(frame_count.0);
    let Ok(nif_index) = nif_index_q.get(event.entity) else {
        error!("Race preview skeleton was missing nif node index!");
        return;
    };
    let head_entity_opt = get_bone_entity(nif_index, "Bip01 Head");
    if let Some(head_bone_entity) = head_entity_opt {
        // We trigger this rather than just doing the work ourselves because
        commands
            .entity(head_bone_entity)
            .trigger(PreviewHeadBoneEntityReady);

        commands
            .entity(head_bone_entity)
            .insert(PreviewHeadBoneEntity);
        set_head_rotation_to_slider(
            &mut slider_thumb,
            0.0,
            &global_transforms_q,
            head_bone_entity,
            &mut race_preview_camera,
            &mut race_preview_scene,
        );
    }
}
