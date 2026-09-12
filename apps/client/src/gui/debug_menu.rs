use bevy::prelude::*;
use esm_parser_lib::RefId;

use crate::{defs::LocalPlayer, mechanics::creature_stats::CreatureStats};
#[derive(Component, Default, Clone)]
pub struct PlayerDebugStatText(pub RefId);
#[derive(Component, Default, Clone)]
pub struct PlayerDebugStat(pub RefId);
pub fn spawn_debug_menu(stat_node_entities: Vec<Entity>, commands: &mut Commands) {
    let debug_menu = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                left: px(10.0),
                top: px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 1.0)),
        ))
        .with_children(|root| {
            root.spawn(Text::new("Chris birch's cheat menu (only for him)"));
        })
        .id();

    for stat_node_entity in stat_node_entities {
        commands.entity(debug_menu).add_child(stat_node_entity);
    }
}

pub fn debug_stat_modifier(stat: RefId, stat_value: f32, commands: &mut Commands) -> Entity {
    let stat_name = stat.resolve_string();
    commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .with_children(|root| {
            root.spawn((Button, Text::new(" - "), PlayerDebugStat(stat.clone())))
                .observe(on_subtract);
            root.spawn((
                PlayerDebugStatText(stat.clone()),
                Text::new(format!("{}: {}", stat_name, stat_value)),
            ));
            root.spawn((Button, Text::new(" + "), PlayerDebugStat(stat)))
                .observe(on_add_hold)
                .observe(on_add);
        })
        .id()
}
pub fn spawn_debug_menu_scene(
    player_creature_stats: Single<&CreatureStats, With<LocalPlayer>>,
    mut commands: Commands,
) {
    let mut debug_stat_entities = Vec::new();
    for attr in &player_creature_stats.attributes {
        debug_stat_entities.push(debug_stat_modifier(
            attr.0.clone(),
            attr.1.get_modified(),
            &mut commands,
        ));
    }
    spawn_debug_menu(debug_stat_entities, &mut commands);
}
fn on_subtract(
    ev: On<Pointer<Press>>,
    child_of_q: Query<(&ChildOf, &PlayerDebugStat)>,
    children_q: Query<&Children>,
    player_creature_stats: Single<&mut CreatureStats, With<LocalPlayer>>,
    player_debug_stat_text_q: Query<&mut Text, With<PlayerDebugStatText>>,
) {
    get_and_modify(
        ev.entity,
        child_of_q,
        children_q,
        player_creature_stats,
        player_debug_stat_text_q,
        -1.0,
    );
}
fn on_add(
    ev: On<Pointer<Press>>,
    child_of_q: Query<(&ChildOf, &PlayerDebugStat)>,
    children_q: Query<&Children>,
    player_creature_stats: Single<&mut CreatureStats, With<LocalPlayer>>,
    player_debug_stat_text_q: Query<&mut Text, With<PlayerDebugStatText>>,
) {
    get_and_modify(
        ev.entity,
        child_of_q,
        children_q,
        player_creature_stats,
        player_debug_stat_text_q,
        1.0,
    );
}
fn on_add_hold(
    ev: On<Pointer<Drag>>,
    child_of_q: Query<(&ChildOf, &PlayerDebugStat)>,
    children_q: Query<&Children>,
    player_creature_stats: Single<&mut CreatureStats, With<LocalPlayer>>,
    player_debug_stat_text_q: Query<&mut Text, With<PlayerDebugStatText>>,
) {
    get_and_modify(
        ev.entity,
        child_of_q,
        children_q,
        player_creature_stats,
        player_debug_stat_text_q,
        1.0,
    );
}
fn get_and_modify(
    entity: Entity,
    child_of_q: Query<(&ChildOf, &PlayerDebugStat)>,
    children_q: Query<&Children>,
    mut player_creature_stats: Single<&mut CreatureStats, With<LocalPlayer>>,
    mut player_debug_stat_text_q: Query<&mut Text, With<PlayerDebugStatText>>,
    change: f32,
) {
    let (child_of, player_debug_stat) = child_of_q.get(entity).unwrap();
    let children = children_q.get(child_of.0).unwrap();
    let text_entity = children[1];
    let mut text = player_debug_stat_text_q.get_mut(text_entity).unwrap();

    let player_stat = player_creature_stats
        .attributes
        .get_mut(&player_debug_stat.0)
        .unwrap();
    player_stat.set_base(player_stat.base + change, false);
    text.0 = format!("{}: {}", player_debug_stat.0.to_string(), player_stat.base);
}
