use bevy::{
    app::Propagate,
    camera::{RenderTarget, visibility::RenderLayers},
    prelude::*,
    render::render_resource::TextureFormat,
};
use bevy_nif::{
    attach_parts::AttachmentType,
    hash_str,
    spawner::{NifInstantiated, NifNodeIndex, NifScene},
};
use esm_parser_lib::{
    RefId,
    types::{
        bodypart::{Bodypart, BodypartFlags, BodypartId, BodypartType},
        flags::{NpcFlags, RaceFlags},
        race::Race,
    },
};
use mwphysics::MW_UNITS_PER_METER;

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
pub struct RaceSelectPreviewCamera;
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
#[derive(Component)]
pub struct SliderTrack;
#[derive(Component)]
pub struct SliderThumb;
const PREVIEW_WINDOW_PX: f32 = 256.0;
const ROTATION_SLIDER_THUMB_WIDTH: f32 = 30.0;
pub fn setup_race_select(
    mut commands: Commands,
    esm_store: Res<EsmStore>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(default_player) = esm_store.npcs.get(&RefId::from_str("player")) else {
        panic!("missing player def in esm file!")
    };
    // TODO:: get and set player's current race, for now just default to index 0 on available races
    let available_races = get_playable_races(&esm_store.races);
    let default_race_refid = default_player.race.clone();

    let Some(default_race) = esm_store.races.get(&default_race_refid) else {
        panic!("could not get race def from default race!");
    };
    let available_heads = get_body_parts(
        BodypartId::Head,
        &esm_store.bodyparts,
        &default_race_refid,
        default_player.is_male(),
    );
    let available_hairs = get_body_parts(
        BodypartId::Hair,
        &esm_store.bodyparts,
        &default_race_refid,
        default_player.is_male(),
    );
    let mut spell_list_id = Entity::PLACEHOLDER;
    let mut skill_list_id = Entity::PLACEHOLDER;

    let image = Image::new_target_texture(
        512,
        512,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );

    let preview_image_handle = images.add(image);
    // 1. Spawn the camera
    commands.spawn((
        RaceSelectPreviewCamera,
        Camera3d::default(),
        Camera {
            order: -1,
            ..default()
        },
        RenderTarget::Image(preview_image_handle.clone().into()),
        AmbientLight {
            color: Color::linear_rgb(1.0, 1.0, 1.0),
            brightness: 400.0,
            affects_lightmapped_meshes: true,
        },
        Transform::from_xyz(0.0, 1.8, 0.6),
        RenderLayers::layer(1),
    ));

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
                    // Race preview
                    left_col
                        .spawn(Node {
                            width: Val::Px(PREVIEW_WINDOW_PX),
                            height: Val::Px(256.0),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        })
                        .with_children(|race_preview_node| {
                            race_preview_node.spawn((
                                // Race preview image
                                Node {
                                    width: Val::Px(256.0),
                                    height: Val::Px(236.0),
                                    ..default()
                                },
                                ImageNode {
                                    image: preview_image_handle,
                                    color: Color::WHITE,
                                    ..default()
                                },
                            ));
                            race_preview_node
                                .spawn(
                                    // rotation slider bar
                                    (
                                        Node {
                                            width: Val::Px(256.0),
                                            height: Val::Px(20.0),
                                            justify_content: JustifyContent::Start,
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgb(0.1, 0.1, 0.1).into()),
                                        SliderTrack,
                                    ),
                                )
                                // rotation slider thumb
                                .with_children(|slider_track| {
                                    slider_track
                                        .spawn((
                                            Node {
                                                width: Val::Px(ROTATION_SLIDER_THUMB_WIDTH),
                                                height: Val::Px(20.0),
                                                position_type: PositionType::Absolute, // Allows moving along the track
                                                left: Val::Px(PREVIEW_WINDOW_PX / 1.8), // Start position
                                                ..default()
                                            },
                                            BackgroundColor(Color::srgb(0.6, 0.6, 0.6).into()),
                                            SliderThumb,
                                        ))
                                        .observe(rotation_slider_dragged)
                                        .observe(on_rotation_slider_drag_start);
                                });
                        });
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
                            parent_panel
                                .spawn(generic_button((), "next sex"))
                                .observe(change_sex);
                            parent_panel
                                .spawn(generic_button((), "previous sex"))
                                .observe(change_sex);
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
    dbg!(race_select_ui.is_male);
    commands.entity(race_select_panel).insert(race_select_ui);
    commands.run_system_cached(respawn_preview);
    commands.run_system_cached(redraw_race_select_ui);
}
#[derive(Component)]
pub struct RacePreviewScene;
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
    let is_male = race_select_ui.is_male;
    recount_bodyparts(&esm_store.bodyparts, &mut race_select_ui, is_male);
    commands.run_system_cached(respawn_preview);
    commands.run_system_cached(redraw_race_select_ui);
}
fn recount_bodyparts(bodyparts: &Store<Bodypart>, race_select_ui: &mut RaceSelectUI, male: bool) {
    dbg!(male);
    let available_heads = get_body_parts(
        BodypartId::Head,
        bodyparts,
        &race_select_ui.current_race_refid,
        male,
    );
    let available_hairs = get_body_parts(
        BodypartId::Hair,
        bodyparts,
        &race_select_ui.current_race_refid,
        male,
    );
    dbg!(&available_heads);
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
fn change_sex(
    _event: On<Pointer<Press>>,
    mut race_select_ui: Single<&mut RaceSelectUI>,
    mut commands: Commands,
    esm_store: Res<EsmStore>,
) {
    race_select_ui.is_male = !race_select_ui.is_male;
    dbg!(race_select_ui.is_male);
    let is_male = race_select_ui.is_male;
    recount_bodyparts(&esm_store.bodyparts, &mut race_select_ui, is_male);
    commands.run_system_cached(respawn_preview);
}

fn get_body_parts(
    part_id: BodypartId,
    bodyparts: &Store<Bodypart>,
    current_race_id: &RefId,
    male: bool,
) -> Vec<RefId> {
    let mut found_parts = Vec::new();
    for bodypart in bodyparts.iter_shared() {
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
        if male == is_female_part {
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
    old_nif_scenes: Query<Entity, With<RacePreviewScene>>,
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
            Name::new("race select preview mesh"),
            Transform::default(),
            Visibility::Visible,
            // Render to the preview window
            Propagate(RenderLayers::layer(1)),
            RacePreviewScene,
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
    //TODO:: loop around
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
    let asset_handle = asset_server.load("data/base_anim.nif");
    let skeleton_entity = commands
        .spawn((
            NifScene(asset_handle),
            AttachmentType::MainSkeleton { skeleton_id },
            Visibility::Inherited,
            transform,
        ))
        .observe(on_preview_skeleton_loaded)
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

    for (path, attachment_type) in paths_with_target_bone {
        let asset_handle = asset_server.load(path.to_string());
        let child_second = commands
            .spawn((
                NifScene(asset_handle),
                attachment_type,
                Transform::default(),
                Visibility::Inherited,
            ))
            .observe(on_preview_head_loaded)
            .id();
        commands.entity(skeleton_entity).add_child(child_second);
    }
    skeleton_entity
}
const LOOK_AT_OFFSET: Vec3 = Vec3::new(0.0, 5.0 / MW_UNITS_PER_METER, 0.0);
const PREVIEW_CAMERA_OFFSET: Vec3 =
    Vec3::new(0.0, 5.0 / MW_UNITS_PER_METER, -30.0 / MW_UNITS_PER_METER);
#[derive(Component)]
pub struct PreviewHeadBoneEntity(pub Entity);
pub fn on_preview_head_loaded(
    _event: On<NifInstantiated>,
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
    head_entity: Single<&PreviewHeadBoneEntity>,
) {
    set_head_rotation_to_slider(
        &mut slider_thumb,
        0.0,
        &global_transforms_q,
        head_entity.0,
        &mut race_preview_camera,
        &mut race_preview_scene,
    );
}
pub fn on_preview_skeleton_loaded(
    event: On<NifInstantiated>,

    mut commands: Commands,
    // Query for the scene root and the camera
    mut camera_query: Single<&mut Transform, With<RaceSelectPreviewCamera>>,
    nif_index_q: Query<&NifNodeIndex>,
    global_transforms_q: Query<&GlobalTransform>,
) {
    let Ok(nif_index) = nif_index_q.get(event.entity) else {
        error!("Race preview skeleton was missing nif node index!");
        return;
    };
    const BIP_01_HASH: u64 = hash_str("Bip01 Head");
    if let Some(head_entity) = nif_index.named_nodes.get(&BIP_01_HASH) {
        if let Ok(head_global_transform) = global_transforms_q.get(*head_entity) {
            // Snap camera to the correct position IMMEDIATELY
            let head_pos = head_global_transform.translation();
            camera_query.translation = head_pos + PREVIEW_CAMERA_OFFSET;
            camera_query.look_at(head_pos + LOOK_AT_OFFSET, Vec3::Y);

            // Tag the entity so we don't keep searching for it
            commands
                .entity(event.entity)
                .insert(PreviewHeadBoneEntity(*head_entity));
        }
    }
}
#[derive(Resource, Default)]
pub struct SliderDragStart(pub Vec2);
fn on_rotation_slider_drag_start(
    event: On<Pointer<Press>>,
    thumb: Single<&Node, With<SliderThumb>>,
    mut slider_drag_start: ResMut<SliderDragStart>,
) {
    let current_px = match thumb.left {
        Val::Px(px) => px,
        _ => 0.0,
    };
    // How far is the mouse from the thumb's current left edge?
    slider_drag_start.0.x = event.pointer_location.position.x - current_px;
}
fn rotation_slider_dragged(
    event: On<Pointer<Drag>>,
    slider_drag_start: Res<SliderDragStart>,
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
    // So we know how far we overshot how far we can drag the slider
    head_entity: Single<&PreviewHeadBoneEntity>,
) {
    let relative_mouse_x = event.pointer_location.position.x - slider_drag_start.0.x;
    const MAX_PX: f32 = PREVIEW_WINDOW_PX - ROTATION_SLIDER_THUMB_WIDTH;
    let new_px = relative_mouse_x.clamp(0.0, MAX_PX);

    // Apply Rotation
    let t = new_px / MAX_PX;
    let angle = t * std::f32::consts::TAU;
    race_preview_scene.rotation = Quat::from_rotation_y(angle);
    // Update UI
    slider_thumb.left = Val::Px(new_px);

    // Camera logic (Moved to its own system later if possible)
    if let Ok(head_global) = global_transforms_q.get(head_entity.0) {
        let head_pos = head_global.translation();
        race_preview_camera.translation = head_pos + PREVIEW_CAMERA_OFFSET;
        race_preview_camera.look_at(head_pos + LOOK_AT_OFFSET, Vec3::Y);
    }
}

fn set_head_rotation_to_slider(
    slider_thumb: &mut Node,
    delta: f32,
    global_transforms_q: &Query<&GlobalTransform>,
    head_entity: Entity,
    race_preview_camera_transform: &mut Transform,
    race_preview_scene_transform: &mut Transform,
) -> f32 {
    // Get the current position
    let current_px = match slider_thumb.left {
        Val::Px(px) => px,
        _ => 0.0,
    };
    const MAX_PX: f32 = PREVIEW_WINDOW_PX - ROTATION_SLIDER_THUMB_WIDTH;
    let unclamped = current_px + delta;
    let new_px = (current_px + delta).clamp(0.0, MAX_PX);
    let overshoot = unclamped - new_px;
    //Map pixel range [0, max_px] to [-0.5, 0.5]
    let t = new_px / MAX_PX;
    let angle = t * std::f32::consts::TAU;
    if let Ok(head_global_transform) = global_transforms_q.get(head_entity) {
        let head_pos = head_global_transform.translation();
        let target = head_pos + LOOK_AT_OFFSET;
        let camera_pos = head_pos + PREVIEW_CAMERA_OFFSET;
        race_preview_camera_transform.translation = camera_pos;
        race_preview_camera_transform.look_at(target, Vec3::Y);
    }
    race_preview_scene_transform.rotation = Quat::from_rotation_y(angle);

    slider_thumb.left = Val::Px(new_px);
    return overshoot;
}
