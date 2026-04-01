use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
use bevy_nif::{attach_parts::AttachmentType, spawner::NifScene};
use esm_parser_lib::{
    RefId,
    types::{
        bodypart::{BodypartFlags, BodypartId, BodypartType},
        flags::RaceFlags,
    },
};
use tes3mp_rust::defs::{BodypartDefinitions, RaceDefinitions};

use crate::{
    NextAvailableSkeletonId,
    const_defs::NIF_SCALE_MULTIPLIER,
    gui::{RenderPosition, UI_BACKGROUND, generic_button::generic_button},
    physics::CreatureControllerLink,
};
#[derive(Component)]
pub struct RaceSelectUI {
    gender_index: usize,
    face_index: usize,
    hair_index: usize,
    playable_race_ids: Vec<RefId>,
    current_race_id: RefId,
    available_heads: Vec<RefId>,
    available_hairs: Vec<RefId>,
}
#[derive(Component)]
pub struct RaceSelectButtonIndex(usize);
pub fn setup_race_select(
    mut commands: Commands,
    bodyparts: Res<BodypartDefinitions>,
    race_defs: Res<RaceDefinitions>,
) {
    // TODO:: get and set player's current race, for now just default to index 0 on available races
    let available_races = get_playable_races(&race_defs);
    let default_race_id = available_races[0].clone();
    let available_heads = get_body_parts(BodypartId::Head, &bodyparts, &default_race_id);
    let available_hairs = get_body_parts(BodypartId::Hair, &bodyparts, &default_race_id);
    let race_select_panel = commands
        .spawn((
            Node {
                width: Val::Percent(70.0),
                height: Val::Percent(80.0),
                position_type: PositionType::Absolute,
                left: Val::Percent(20.0),
                top: Val::Percent(10.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::NoWrap,
                align_items: AlignItems::Start,
                justify_content: JustifyContent::Start,
                ..default()
            },
            ZIndex(RenderPosition::Background.as_i32()),
            Pickable::default(),
            BackgroundColor(UI_BACKGROUND),
        ))
        .with_children(|parent_panel| {
            for index in 0..available_races.len() {
                parent_panel
                    .spawn(generic_button(RaceSelectButtonIndex(index)))
                    .observe(race_select_button_pressed);
            }
            parent_panel
                .spawn(generic_button(()))
                .observe(race_select_left_pressed);
            parent_panel
                .spawn(generic_button(()))
                .observe(race_select_right_pressed);
        })
        .id();
    let race_select_ui = RaceSelectUI {
        gender_index: 0,
        face_index: 0,
        hair_index: 0,
        current_race_id: default_race_id,
        playable_race_ids: available_races,
        available_heads,
        available_hairs,
    };
    commands.entity(race_select_panel).insert(race_select_ui);
}
// FOR TESTING ONLY
#[derive(Component)]
pub struct OldNifScene;
fn race_select_button_pressed(
    event: On<Pointer<Press>>,
    button_indices: Query<&RaceSelectButtonIndex>,
    mut race_select_ui: Single<&mut RaceSelectUI>,
    bodypart_defs: Res<BodypartDefinitions>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    old_nif_scenes: Query<Entity, With<OldNifScene>>,
    mut next_available_skeleton_id: ResMut<NextAvailableSkeletonId>,
) {
    for old_nif_scene in old_nif_scenes {
        commands.entity(old_nif_scene).despawn();
    }
    let Ok(button_index) = button_indices.get(event.entity) else {
        return;
    };
    race_select_ui.current_race_id = race_select_ui.playable_race_ids[button_index.0].clone();
    recount_bodyparts(&bodypart_defs, &mut race_select_ui);
    let prepend_path = |path: String| -> String {
        let mut data_path = "data/Data Files/Meshes/".to_string();
        data_path.push_str(&path);
        if data_path.to_lowercase().ends_with(".nif") {
            let len = data_path.len();
            data_path.replace_range(len - 4.., ".nif");
        }
        data_path
    };
    let path_hair = bodypart_defs
        .0
        .get(&race_select_ui.available_hairs[0])
        .unwrap()
        .model
        .clone();
    let path_hair = prepend_path(path_hair);
    let path_head = bodypart_defs
        .0
        .get(&race_select_ui.available_heads[0])
        .unwrap()
        .model
        .clone();
    let path_head = prepend_path(path_head);
    let entity = commands
        .spawn((
            Transform {
                translation: Vec3::new(0.0, 0.0, 0.0),
                ..default()
            },
            OldNifScene,
        ))
        .id();
    spawn_mesh(
        entity,
        commands,
        &asset_server,
        next_available_skeleton_id.0,
        Transform {
            translation: Vec3::new(0.0, 0.0, 0.0),
            rotation: Quat::default(),
            scale: Vec3::splat(NIF_SCALE_MULTIPLIER),
        },
        &path_hair,
        &path_head,
    );
    next_available_skeleton_id.0 += 1;
}
fn recount_bodyparts(bodypart_defs: &BodypartDefinitions, race_select_ui: &mut RaceSelectUI) {
    let available_heads = get_body_parts(
        BodypartId::Head,
        &bodypart_defs,
        &race_select_ui.current_race_id,
    );
    let available_hairs = get_body_parts(
        BodypartId::Hair,
        &bodypart_defs,
        &race_select_ui.current_race_id,
    );
    race_select_ui.available_heads = available_heads;
    race_select_ui.available_hairs = available_hairs;
}
fn race_select_left_pressed(
    _event: On<Pointer<Press>>,
    mut race_select_ui: Single<&mut RaceSelectUI>,
    race_defs: Res<RaceDefinitions>,
) {
    dbg!("left");

    dbg!(race_defs.0.get(&race_select_ui.current_race_id));
}
fn race_select_right_pressed(_event: On<Pointer<Press>>) {}

fn get_body_parts(
    part_id: BodypartId,
    bodyparts: &BodypartDefinitions,
    current_race_id: &RefId,
) -> Vec<RefId> {
    let gender_index = 0;
    let mut found_parts = Vec::new();
    for (_refid, bodypart) in &bodyparts.0 {
        if bodypart.data.flags.contains(BodypartFlags::NOT_PLAYABLE) {
            continue;
        }
        if bodypart.data.bodypart_type != BodypartType::Skin {
            continue;
        }
        if bodypart.data.part != part_id {
            continue;
        }
        let is_female_part = bodypart.data.flags.contains(BodypartFlags::FEMALE);
        if (gender_index == 1) != is_female_part {
            continue;
        }
        if bodypart.is_first_person_bodypart() {
            continue;
        }
        if bodypart.race == *current_race_id {
            found_parts.push(bodypart.id.clone());
        }
    }

    found_parts
}
fn get_playable_races(race_defs: &RaceDefinitions) -> Vec<RefId> {
    // TODO:: get and set player's current race, for now just default to index 0 on available races
    // should pass in current race's ref id here
    let mut playable_races = Vec::new();
    for (_ref_id, race) in &race_defs.0 {
        if !race.data.flags.contains(RaceFlags::PLAYABLE) {
            continue;
        }
        playable_races.push(race.id.clone());
    }
    //TODO:: rest of race.cpp in mwgui
    playable_races
}
fn spawn_mesh(
    entity: Entity,
    mut commands: Commands,
    asset_server: &Res<AssetServer>,
    skeleton_id: u64,
    transform: Transform,
    hair_path: &str,
    head_path: &str,
) -> Entity {
    dbg!(skeleton_id);
    let asset_handle = asset_server.load("data/base_anim.nif");
    let skeleton_entity = commands
        .spawn((
            NifScene(asset_handle),
            AttachmentType::MainSkeleton { skeleton_id },
            transform,
        ))
        .id();
    commands.entity(entity).add_child(skeleton_entity);
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

    for (path, mut attachment_type) in paths_with_target_bone {
        let asset_handle = asset_server.load(path.to_string());
        if let AttachmentType::DoubleSidedRigid {
            target_bone,
            skeleton_id,
        } = attachment_type.clone()
        {
            attachment_type = AttachmentType::Rigid {
                target_bone: format!("Left {}", target_bone),
                skeleton_id,
            };
            let child = commands
                .spawn((
                    NifScene(asset_handle.clone()),
                    attachment_type,
                    transform,
                    Visibility::Inherited,
                ))
                .id();
            commands.entity(skeleton_entity).add_child(child);
            attachment_type = AttachmentType::Rigid {
                target_bone: format!("Right {}", target_bone),
                skeleton_id,
            };
        }
        let child_second = commands
            .spawn((
                NifScene(asset_handle),
                attachment_type,
                Transform::default(),
                Visibility::Inherited,
            ))
            .id();
        commands.entity(skeleton_entity).add_child(child_second);
    }
    skeleton_entity
}
