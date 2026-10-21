use bevy::{
    app::Propagate,
    camera::{RenderTarget, visibility::RenderLayers},
    diagnostic::FrameCount,
    prelude::*,
    render::render_resource::TextureFormat,
};
use bevy_nif::loader::Nif;
use common::consts::MW_UNITS_PER_METER;
use common::state::CharCreationState;
use esm_parser_lib::{
    RefId,
    types::{
        bodypart::{Bodypart, BodypartFlags, BodypartId, BodypartType},
        flags::{NpcFlags, RaceFlags},
        race::Race,
    },
};
use esm_store::{EsmStore, Store};

use crate::{
    defs::{ColliderEntityLink, LocalPlayer},
    gui::{
        RenderPosition,
        generic_button::generic_button,
        race_select_character_preview::{PreviewHeadBoneEntity, spawn_race_select_preview},
        spell_description::{
            on_out_hide_tooltip, update_spell_description_tooltip, update_tooltip_position,
        },
    },
    setup::setup_npc::{NpcMesh, resolve_body_parts, spawn_npc},
    shared_defs::skeletons::NextAvailableSkeletonId,
    utils::prepend_mesh_data_path,
};

#[derive(Component)]
pub struct RaceSelectPreviewCamera;
#[derive(Component)]
pub struct RaceSelectUI {
    preview_skeleton_handle: Handle<Nif>,
    is_male: bool,
    head_index: usize,
    hair_index: usize,
    playable_race_refids: Vec<RefId>,
    current_race_refid: RefId,
    available_heads: Vec<RefId>,
    available_hairs: Vec<RefId>,
    // Just to keep all the nifs loaded
    race_select_handles: Vec<Handle<Nif>>,
}
#[derive(Component, Clone, Default)]
pub struct RaceSelectSpellPowerPanel;
#[derive(Component, Clone, Default)]
pub struct RaceSelectSpellPower(pub RefId);
#[derive(Component, Clone, Default)]
pub struct RaceSelectSkillPanel;
#[derive(Component, Clone, Default)]
pub struct RaceSelectSkill;

#[derive(Component)]
pub struct RaceSelectButtonIndex(usize);
#[derive(Component, Clone, Default)]
pub struct SliderTrack;
#[derive(Component, Clone, Default)]
pub struct SliderThumb;
const PREVIEW_WINDOW_PX: f32 = 256.0;
const ROTATION_SLIDER_THUMB_WIDTH: f32 = 30.0;
const SLIDER_TRACK_START_POS: f32 = PREVIEW_WINDOW_PX / 1.8;
const ROTATION_SLIDER_MAX_PX: f32 = PREVIEW_WINDOW_PX - ROTATION_SLIDER_THUMB_WIDTH;
pub fn setup_race_select(
    mut commands: Commands,
    esm_store: Res<EsmStore>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
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
    // Spawn the preview camera
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
    let preview_area = commands
        .spawn_scene(preview_area(preview_image_handle))
        .id();
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
        .id();
    let left_col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(70.0),
            row_gap: Val::Px(20.0),
            ..default()
        })
        .id();
    commands.entity(preview_area).insert(ChildOf(left_col));
    commands.entity(left_col).with_children(|left_col| {
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
    let right_col = commands
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
            right_col
                .spawn(generic_button((), "Ok"))
                .observe(on_race_accepted);
        })
        .id();

    commands.entity(left_col).insert(ChildOf(race_select_panel));
    commands
        .entity(right_col)
        .insert(ChildOf(race_select_panel));
    let skeleton_asset_handle = asset_server.load("data/base_anim.nif");
    let mut race_select_handles = Vec::new();
    for i in 0..available_hairs.len() {
        let hair = &available_hairs[i];
        let hair_path = esm_store.bodyparts.get(hair).unwrap().model.clone();
        let hair_path = prepend_mesh_data_path(hair_path);
        race_select_handles.push(asset_server.load(hair_path));
    }
    for i in 0..available_heads.len() {
        let head = &available_heads[i];
        let head_path = esm_store.bodyparts.get(head).unwrap().model.clone();
        let head_path = prepend_mesh_data_path(head_path);
        race_select_handles.push(asset_server.load(head_path));
    }

    // dbg!(&race_select_handles);
    let race_select_ui = RaceSelectUI {
        preview_skeleton_handle: skeleton_asset_handle,
        is_male: !default_player.npc_flags.contains(NpcFlags::FEMALE),
        head_index: 0,
        hair_index: 0,
        current_race_refid: default_race_refid,
        playable_race_refids: available_races,
        available_heads,
        available_hairs,
        race_select_handles,
    };
    commands.entity(race_select_panel).insert(race_select_ui);
    commands.run_system_cached(respawn_preview);
    commands.run_system_cached(redraw_race_select_ui);
}
fn preview_area(handle: Handle<Image>) -> impl Scene {
    let background_color = Color::srgb(0.6, 0.6, 0.6);
    bsn! {
        Node { width: px(PREVIEW_WINDOW_PX), height: px(256), flex_direction: FlexDirection::Column }
        Children [
            Node {
                width: Val::Px(256.0),
                height: Val::Px(236.0),
            }
            ImageNode { image: {handle}, color: Color::WHITE }
            --
            Node {
                width: Val::Px(256.0),
                height: Val::Px(20.0),
                justify_content: JustifyContent::Start,
            }
        BackgroundColor(Color::srgb(0.1, 0.1, 0.1))
            SliderTrack
            Children [
            Node {
                width: Val::Px(ROTATION_SLIDER_THUMB_WIDTH),
                height: Val::Px(20.0),
                position_type: PositionType::Absolute, // Allows moving along the track
                left: Val::Px(SLIDER_TRACK_START_POS), // Start position
            }
        BackgroundColor(background_color)
            SliderThumb
            on(rotation_slider_dragged)
            on(on_rotation_slider_drag_start)
            ]
            ]
    }
}
pub fn despawn_race_select_panel(
    race_select_panel_entity: Single<Entity, With<RaceSelectUI>>,
    race_select_preview_camera: Single<Entity, With<RaceSelectPreviewCamera>>,
    mut commands: Commands,
) {
    commands.entity(*race_select_panel_entity).despawn();
    commands.entity(*race_select_preview_camera).despawn();
}
fn on_race_accepted(
    _event: On<PointerClick>,
    mut char_creation_state: ResMut<NextState<CharCreationState>>,
    esm_store: Res<EsmStore>,
    race_select_ui: Single<&RaceSelectUI>,
    _player_location: Single<&GlobalTransform, With<LocalPlayer>>,
    asset_server: Res<AssetServer>,
    mut next_available_skeleton_id: ResMut<NextAvailableSkeletonId>,
    mut commands: Commands,
    // TEMPORARY
    player_mesh: Single<Entity, With<NpcMesh>>,
    player_components: Single<(Entity, &ColliderEntityLink), With<LocalPlayer>>,
) {
    let hair_path = esm_store
        .bodyparts
        .get(&race_select_ui.available_hairs[race_select_ui.hair_index])
        .unwrap()
        .model
        .clone();
    let hair_path = prepend_mesh_data_path(hair_path);
    let head_path = esm_store
        .bodyparts
        .get(&race_select_ui.available_heads[race_select_ui.head_index])
        .unwrap()
        .model
        .clone();
    let head_path = prepend_mesh_data_path(head_path);

    let parts = resolve_body_parts(
        &race_select_ui.current_race_refid,
        !race_select_ui.is_male,
        false,
        false,
        &esm_store,
    );
    // TEMPORARY
    commands.entity(*player_mesh).despawn();

    spawn_npc(
        player_components.0,
        &mut commands,
        &asset_server,
        next_available_skeleton_id.next(),
        parts,
        head_path,
        hair_path,
        //TEMPORARY
        true,
    );

    char_creation_state.set(CharCreationState::ChoosingClass);
}
#[derive(Component)]
pub struct RacePreviewScene;
fn race_select_button_pressed(
    event: On<PointerClick>,
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
    race_select_ui.available_heads = available_heads;
    race_select_ui.available_hairs = available_hairs;
    race_select_ui.head_index = 0;
    race_select_ui.hair_index = 0;
}
fn head_select_left_pressed(
    _event: On<PointerClick>,
    mut race_select_ui: Single<&mut RaceSelectUI>,
    mut commands: Commands,
) {
    race_select_ui.head_index =
        (race_select_ui.head_index + 1) % race_select_ui.available_heads.len();
    commands.run_system_cached(respawn_preview);
}
fn head_select_right_pressed(
    _event: On<PointerClick>,
    mut race_select_ui: Single<&mut RaceSelectUI>,
    mut commands: Commands,
) {
    let len = race_select_ui.available_heads.len();
    race_select_ui.head_index = (race_select_ui.head_index + len - 1) % len;
    commands.run_system_cached(respawn_preview);
}
fn hair_select_left_pressed(
    _event: On<PointerClick>,
    mut race_select_ui: Single<&mut RaceSelectUI>,
    mut commands: Commands,
) {
    race_select_ui.hair_index =
        (race_select_ui.hair_index + 1) % race_select_ui.available_hairs.len();
    commands.run_system_cached(respawn_preview);
}
fn hair_select_right_pressed(
    _event: On<PointerClick>,
    mut race_select_ui: Single<&mut RaceSelectUI>,
    mut commands: Commands,
) {
    let len = race_select_ui.available_hairs.len();
    race_select_ui.hair_index = (race_select_ui.hair_index + len - 1) % len;
    commands.run_system_cached(respawn_preview);
}
fn change_sex(
    _event: On<PointerClick>,
    mut race_select_ui: Single<&mut RaceSelectUI>,
    mut commands: Commands,
    esm_store: Res<EsmStore>,
) {
    race_select_ui.is_male = !race_select_ui.is_male;
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
        // Hide any unused power children nodes
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
    slider_thumb: Single<&Node, With<SliderThumb>>,
    old_nif_scenes: Query<Entity, With<RacePreviewScene>>,
    esm_store: Res<EsmStore>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut next_available_skeleton_id: ResMut<NextAvailableSkeletonId>,
) {
    for old_nif_scene in old_nif_scenes {
        commands
            .delayed()
            .secs(0.01)
            .entity(old_nif_scene)
            .despawn();
    }

    let path_hair = esm_store
        .bodyparts
        .get(&race_select_ui.available_hairs[race_select_ui.hair_index])
        .unwrap()
        .model
        .clone();
    let path_hair = prepend_mesh_data_path(path_hair);
    let path_head = esm_store
        .bodyparts
        .get(&race_select_ui.available_heads[race_select_ui.head_index])
        .unwrap()
        .model
        .clone();
    let path_head = prepend_mesh_data_path(path_head);
    // Pre rotate the preview mesh according to the starting rotation of the slider
    let current_px = match slider_thumb.left {
        Val::Px(px) => px,
        _ => 0.0,
    };
    let t = current_px / ROTATION_SLIDER_MAX_PX;

    let angle = t * std::f32::consts::TAU;
    let race_preview_entity = commands
        .spawn((
            Name::new("race select preview mesh"),
            Transform::from_rotation(Quat::from_rotation_y(angle)),
            Visibility::Hidden,
            // Render to the preview window (propagate makes all children layer 1 as well)
            Propagate(RenderLayers::layer(1)),
            RacePreviewScene,
        ))
        .id();
    commands
        .delayed()
        .secs(0.01)
        .entity(race_preview_entity)
        .insert(Visibility::Visible);
    spawn_race_select_preview(
        race_select_ui.preview_skeleton_handle.clone(),
        race_preview_entity,
        Transform::default(),
        next_available_skeleton_id.next(),
        &path_hair,
        &path_head,
        &asset_server,
        &mut commands,
    );
}

const LOOK_AT_OFFSET: Vec3 = Vec3::new(0.0, 5.0 / MW_UNITS_PER_METER, 0.0);
const PREVIEW_CAMERA_OFFSET: Vec3 =
    Vec3::new(0.0, 5.0 / MW_UNITS_PER_METER, -30.0 / MW_UNITS_PER_METER);

#[derive(Resource, Default)]
pub struct SliderDragStart(pub Vec2);
fn on_rotation_slider_drag_start(
    event: On<PointerPress>,
    thumb: Single<&Node, With<SliderThumb>>,
    mut slider_drag_start: ResMut<SliderDragStart>,
) {
    let current_px = match thumb.left {
        Val::Px(px) => px,
        _ => 0.0,
    };
    // How far is the mouse from the thumb's current left edge?
    slider_drag_start.0.x = event.pointer.location().position.x - current_px;
}
fn rotation_slider_dragged(
    event: On<PointerDrag>,
    slider_drag_start: Res<SliderDragStart>,
    mut slider_thumb: Single<&mut Node, With<SliderThumb>>,
    global_transforms_q: Query<&GlobalTransform>,
    mut race_preview_scene: Single<
        &mut Transform,
        (With<RacePreviewScene>, Without<RaceSelectPreviewCamera>),
    >,
    mut race_preview_camera: Single<
        &mut Transform,
        (With<RaceSelectPreviewCamera>, Without<RacePreviewScene>),
    >,
    head_entity: Single<Entity, With<PreviewHeadBoneEntity>>,
) {
    let relative_mouse_x = event.pointer.location().position.x - slider_drag_start.0.x;
    let new_px = relative_mouse_x.clamp(0.0, ROTATION_SLIDER_MAX_PX);

    // Update UI
    slider_thumb.left = Val::Px(new_px);

    // Call shared logic
    update_preview_view(
        new_px / ROTATION_SLIDER_MAX_PX,
        &mut race_preview_scene,
        &mut race_preview_camera,
        *head_entity,
        &global_transforms_q,
    );
}

pub fn set_head_rotation_to_slider(
    slider_thumb: &mut Node,
    delta: f32,
    global_transforms_q: &Query<&GlobalTransform>,
    head_entity: Entity,
    race_preview_camera: &mut Transform,
    race_preview_scene: &mut Transform,
) -> f32 {
    // 1. Calculate the new slider position (The UI logic)
    let current_px = match slider_thumb.left {
        Val::Px(px) => px,
        _ => 0.0,
    };

    let unclamped = current_px + delta;
    let new_px = unclamped.clamp(0.0, ROTATION_SLIDER_MAX_PX);
    let overshoot = unclamped - new_px;

    // 2. Update the UI
    slider_thumb.left = Val::Px(new_px);

    // 3. Update the 3D scene (Shared logic)
    update_preview_view(
        new_px / ROTATION_SLIDER_MAX_PX,
        race_preview_scene,
        race_preview_camera,
        head_entity,
        global_transforms_q,
    );

    // 4. Return the calculated overshoot
    overshoot
}

// Shared logic to update the scene based on a normalized 0.0..1.0 value
fn update_preview_view(
    t: f32,
    race_preview_scene: &mut Transform,
    race_preview_camera: &mut Transform,
    head_entity: Entity,
    global_transforms: &Query<&GlobalTransform>,
) {
    // 1. Calculate rotation
    let angle = t * std::f32::consts::TAU;
    race_preview_scene.rotation = Quat::from_rotation_y(angle);

    // 2. Update Camera
    if let Ok(head_global) = global_transforms.get(head_entity) {
        let head_pos = head_global.translation();
        race_preview_camera.translation = head_pos + PREVIEW_CAMERA_OFFSET;
        race_preview_camera.look_at(head_pos + LOOK_AT_OFFSET, Vec3::Y);
    }
}
pub fn update_race_select_preview_camera(
    global_transforms: Query<&GlobalTransform>,
    mut race_preview_camera: Single<
        &mut Transform,
        (With<RaceSelectPreviewCamera>, Without<RacePreviewScene>),
    >,
    head_entity: Single<Entity, With<PreviewHeadBoneEntity>>,
) {
    if let Ok(head_global) = global_transforms.get(*head_entity) {
        let head_pos = head_global.translation();
        race_preview_camera.translation = head_pos + PREVIEW_CAMERA_OFFSET;
        race_preview_camera.look_at(head_pos + LOOK_AT_OFFSET, Vec3::Y);
    }
}
