use bevy::prelude::*;
use bevy_nif::{attach_parts::AttachmentType, spawner::NifScene};
use esm_parser_lib::{
    RefId,
    types::{
        bodypart::{Bodypart, BodypartFlags, BodypartId, BodypartType},
        flags::{NpcFlags, RaceFlags},
        race::Race,
    },
};

use crate::{
    NextAvailableSkeletonId,
    const_defs::NIF_SCALE_MULTIPLIER,
    esm::{EsmStore, Store},
    gui::{
        RenderPosition,
        generic_button::generic_button,
        spell_description::{
            on_out_hide_tooltip, update_spell_description_tooltip, update_tooltip_position,
        },
    },
};
#[derive(Component)]
pub struct RaceSelectUI {
    is_male: bool,
    head_index: usize,
    hair_index: usize,
    playable_race_refids: Vec<RefId>,
    current_race_refid: RefId,
    available_heads: Vec<RefId>,
    available_hairs: Vec<RefId>,
}
#[derive(Component)]
pub struct RaceSelectSpellPowerPanel;
#[derive(Component)]
pub struct RaceSelectSpellPower(pub RefId);
#[derive(Component)]
pub struct RaceSelectSkillPanel;
#[derive(Component)]
pub struct RaceSelectSkill;

#[derive(Component)]
pub struct RaceSelectButtonIndex(usize);
pub fn setup_race_select(mut commands: Commands, esm_store: Res<EsmStore>) {
    let Some(default_player) = esm_store.npcs.get(&RefId::from_str("player")) else {
        panic!("missing player def in esm file!")
    };
    // TODO:: get and set player's current race, for now just default to index 0 on available races
    let available_races = get_playable_races(&esm_store.races);
    let default_race_refid = default_player.race.clone();

    let Some(default_race) = esm_store.races.get(&default_race_refid) else {
        panic!("could not get race def from default race!");
    };
    let available_heads =
        get_body_parts(BodypartId::Head, &esm_store.bodyparts, &default_race_refid);
    let available_hairs =
        get_body_parts(BodypartId::Hair, &esm_store.bodyparts, &default_race_refid);
    let mut spell_list_id = Entity::PLACEHOLDER;
    let mut skill_list_id = Entity::PLACEHOLDER;

    let race_select_panel = commands
        .spawn((
            Node {
                width: Val::Percent(80.0),
                height: Val::Percent(80.0),
                position_type: PositionType::Absolute,
                left: Val::Percent(10.0),
                top: Val::Percent(5.0),
                flex_direction: FlexDirection::Row, // Main split: Left Side | Right Side (Skills)
                column_gap: Val::Px(20.0),
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            },
            ZIndex(RenderPosition::Background.as_i32()),
            Pickable::default(),
        ))
        .with_children(|main_wrapper| {
            // --- LEFT COLUMN: RACE SELECT (TOP) & POWERS (BOTTOM) ---
            main_wrapper
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Percent(70.0),
                    row_gap: Val::Px(20.0),
                    ..default()
                })
                .with_children(|left_col| {
                    // 1. Race Selection Area (Middle/Top)
                    left_col
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            justify_content: JustifyContent::Start,
                            align_items: AlignItems::Start,
                            ..default()
                        })
                        .with_children(|parent_panel| {
                            for index in 0..available_races.len() {
                                parent_panel
                                    .spawn(generic_button(
                                        RaceSelectButtonIndex(index),
                                        &available_races[index].resolve_string(),
                                    ))
                                    .observe(race_select_button_pressed);
                            }

                            // Head/Hair toggles
                            parent_panel
                                .spawn(generic_button((), "prev head"))
                                .observe(head_select_left_pressed);
                            parent_panel
                                .spawn(generic_button((), "next head"))
                                .observe(head_select_right_pressed);
                            parent_panel
                                .spawn(generic_button((), "prev hair"))
                                .observe(hair_select_left_pressed);
                            parent_panel
                                .spawn(generic_button((), "next hair"))
                                .observe(hair_select_right_pressed);
                        });

                    // 2. Powers Area (Bottom)
                    left_col
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            width: Val::Percent(100.0),
                            border: UiRect::top(Val::Px(2.0)),
                            ..default()
                        })
                        .with_children(|power_section| {
                            power_section.spawn(Text::new("Race Powers"));
                            spell_list_id = power_section
                                .spawn((
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        ..default()
                                    },
                                    RaceSelectSpellPowerPanel,
                                ))
                                .id();
                        });
                });

            // --- RIGHT COLUMN: SKILLS ---
            main_wrapper
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        width: Val::Percent(30.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.8).into()),
                ))
                .with_children(|right_col| {
                    right_col.spawn(Text::new("Skill Bonuses"));
                    skill_list_id = right_col
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                ..default()
                            },
                            RaceSelectSkillPanel,
                        ))
                        .with_children(|skills_column| {
                            for _skill_bonus in &default_race.data.skill_bonuses {
                                skills_column.spawn((RaceSelectSkill, Text::default()));
                            }
                        })
                        .id();
                });
        })
        .id();
    let race_select_ui = RaceSelectUI {
        is_male: !default_player.npc_flags.contains(NpcFlags::FEMALE),
        head_index: 0,
        hair_index: 0,
        current_race_refid: default_race_refid,
        playable_race_refids: available_races,
        available_heads,
        available_hairs,
    };
    commands.entity(race_select_panel).insert(race_select_ui);
    commands.run_system_cached(respawn_preview);
    commands.run_system_cached(redraw_race_select_ui);
}
// FOR TESTING ONLY
#[derive(Component)]
pub struct OldNifScene;
fn race_select_button_pressed(
    event: On<Pointer<Press>>,
    button_indices: Query<&RaceSelectButtonIndex>,
    mut race_select_ui: Single<&mut RaceSelectUI>,
    esm_store: Res<EsmStore>,
    mut commands: Commands,
) {
    let Ok(button_index) = button_indices.get(event.entity) else {
        return;
    };
    race_select_ui.current_race_refid = race_select_ui.playable_race_refids[button_index.0].clone();
    recount_bodyparts(&esm_store.bodyparts, &mut race_select_ui);
    commands.run_system_cached(respawn_preview);
    commands.run_system_cached(redraw_race_select_ui);
}
fn recount_bodyparts(bodyparts: &Store<Bodypart>, race_select_ui: &mut RaceSelectUI) {
    let available_heads = get_body_parts(
        BodypartId::Head,
        bodyparts,
        &race_select_ui.current_race_refid,
    );
    let available_hairs = get_body_parts(
        BodypartId::Hair,
        bodyparts,
        &race_select_ui.current_race_refid,
    );
    race_select_ui.available_heads = available_heads;
    race_select_ui.available_hairs = available_hairs;
    race_select_ui.head_index = 0;
    race_select_ui.hair_index = 0;
}
fn head_select_left_pressed(
    _event: On<Pointer<Press>>,
    mut race_select_ui: Single<&mut RaceSelectUI>,
    mut commands: Commands,
) {
    race_select_ui.head_index =
        (race_select_ui.head_index + 1) % race_select_ui.available_heads.len();
    commands.run_system_cached(respawn_preview);
}
fn head_select_right_pressed(
    _event: On<Pointer<Press>>,
    mut race_select_ui: Single<&mut RaceSelectUI>,
    mut commands: Commands,
) {
    let len = race_select_ui.available_heads.len();
    race_select_ui.head_index = (race_select_ui.head_index + len - 1) % len;
    commands.run_system_cached(respawn_preview);
}
fn hair_select_left_pressed(
    _event: On<Pointer<Press>>,
    mut race_select_ui: Single<&mut RaceSelectUI>,
    mut commands: Commands,
) {
    race_select_ui.hair_index =
        (race_select_ui.hair_index + 1) % race_select_ui.available_hairs.len();
    commands.run_system_cached(respawn_preview);
}
fn hair_select_right_pressed(
    _event: On<Pointer<Press>>,
    mut race_select_ui: Single<&mut RaceSelectUI>,
    mut commands: Commands,
) {
    let len = race_select_ui.available_hairs.len();
    race_select_ui.hair_index = (race_select_ui.hair_index + len - 1) % len;
    commands.run_system_cached(respawn_preview);
}

fn get_body_parts(
    part_id: BodypartId,
    bodyparts: &Store<Bodypart>,
    current_race_id: &RefId,
) -> Vec<RefId> {
    let gender_index = 0;
    let mut found_parts = Vec::new();
    dbg!(bodyparts);
    for bodypart in bodyparts.iter_shared() {
        dbg!(bodypart);
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
fn get_playable_races(races: &Store<Race>) -> Vec<RefId> {
    let mut playable_races = Vec::new();
    for race in races.iter_shared() {
        if !race.data.flags.contains(RaceFlags::PLAYABLE) {
            continue;
        }
        playable_races.push(race.id.clone());
    }
    playable_races.sort_by(|a, b| {
        let name_a = races.get(a).map(|r| &r.name).unwrap();
        let name_b = races.get(b).map(|r| &r.name).unwrap();

        // Use localized/case-insensitive comparison to match OpenMW
        name_a.to_lowercase().cmp(&name_b.to_lowercase())
    });
    playable_races
}
fn redraw_race_select_ui(
    mut commands: Commands,
    esm_store: Res<EsmStore>,
    race_select_ui: Single<&RaceSelectUI>,
    mut spell_powers: Query<(&mut Text, &mut RaceSelectSpellPower), Without<RaceSelectSkill>>,
    mut skills_text: Query<
        (&mut Text, &mut Node),
        (With<RaceSelectSkill>, Without<RaceSelectSpellPower>),
    >,
    power_panel: Single<(Entity, Option<&Children>), With<RaceSelectSpellPowerPanel>>,
    skill_panel: Single<&Children, With<RaceSelectSkillPanel>>,
) {
    let (power_panel_ent, power_children_opt) = *power_panel;
    let skill_children = *skill_panel;

    let Some(current_race) = esm_store.races.get(&race_select_ui.current_race_refid) else {
        return;
    };

    // --- Update Powers ---
    let power_list = &current_race.spells.list;
    let existing_power_children = power_children_opt.map(|c| c.as_ref()).unwrap_or(&[]);
    for (i, power_refid) in power_list.iter().enumerate() {
        let spell_name = esm_store
            .spells
            .get(power_refid)
            .map_or("", |spell| spell.name.as_ref());
        // Update existing
        if let Some(&child) = existing_power_children.get(i) {
            if let Ok((mut text, mut race_power)) = spell_powers.get_mut(child) {
                text.0 = spell_name.to_string();
                race_power.0 = power_refid.clone();
                commands.entity(child).insert(Visibility::Visible);
            }
        } else {
            // Spawn new powers children if there weren't enough children to display all the current powers
            commands.entity(power_panel_ent).with_children(|parent| {
                parent
                    .spawn((
                        RaceSelectSpellPower(power_refid.clone()),
                        Text::new(spell_name),
                    ))
                    .observe(update_tooltip_position::<20>)
                    .observe(update_spell_description_tooltip)
                    .observe(on_out_hide_tooltip);
            });
        }
    }
    for &child in existing_power_children.iter().skip(power_list.len()) {
        commands.entity(child).insert(Visibility::Hidden);
    }
    // --- Update Skills ---
    let skill_bonuses = &current_race.data.skill_bonuses;
    for (i, bonus) in skill_bonuses.iter().enumerate() {
        let is_used = bonus.skill != -1;

        if let Some(&child) = skill_children.get(i) {
            if let Ok((mut text, mut node)) = skills_text.get_mut(child) {
                if is_used {
                    text.0 = format!("{}  {}", bonus.get_skill_id().resolve_string(), bonus.bonus);
                    node.display = Display::Flex; // Show and participate in layout
                    commands.entity(child).insert(Visibility::Visible);
                } else {
                    // unused, collapse it
                    node.display = Display::None;
                    commands.entity(child).insert(Visibility::Hidden);
                }
            }
        }
    }
}
fn respawn_preview(
    race_select_ui: Single<&RaceSelectUI>,
    old_nif_scenes: Query<Entity, With<OldNifScene>>,
    esm_store: Res<EsmStore>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut next_available_skeleton_id: ResMut<NextAvailableSkeletonId>,
) {
    for old_nif_scene in old_nif_scenes {
        commands.entity(old_nif_scene).despawn();
    }
    let prepend_path = |path: String| -> String {
        let mut data_path = "data/Data Files/Meshes/".to_string();
        data_path.push_str(&path);
        if data_path.to_lowercase().ends_with(".nif") {
            let len = data_path.len();
            data_path.replace_range(len - 4.., ".nif");
        }
        data_path
    };
    let path_hair = esm_store
        .bodyparts
        .get(&race_select_ui.available_hairs[race_select_ui.hair_index])
        .unwrap()
        .model
        .clone();
    let path_hair = prepend_path(path_hair);
    let path_head = esm_store
        .bodyparts
        .get(&race_select_ui.available_heads[race_select_ui.head_index])
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
        dbg!(path);
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
