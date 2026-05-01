use bevy::prelude::*;
use bevy_nif::{check_exists, clean_path_common};
use esm_parser_lib::{
    RefId,
    types::{flags::MagicEffectFlags, magic_effect::MagicEffect},
};

use crate::{
    esm::EsmStore,
    gui::{RenderPosition, race_select::RaceSelectSpellPower},
};
#[derive(Component)]
pub struct SpellDescriptionTooltip;
pub fn setup_spell_description_tooltip(mut commands: Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(5.0),
                height: Val::Percent(5.0),
                position_type: PositionType::Absolute,
                left: Val::Percent(20.0),
                top: Val::Percent(10.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Start,
                justify_content: JustifyContent::Start,
                ..default()
            },
            ZIndex(RenderPosition::Tooltip.as_i32()),
            Pickable::default(),
            SpellDescriptionTooltip,
        ))
        .with_children(|spell_tooltip_panel| {
            // Spell name
            spell_tooltip_panel.spawn((
                Node {
                    display: Display::None,
                    ..Default::default()
                },
                Text::default(),
            ));
        });
}
pub fn update_spell_description_tooltip(
    event: On<Pointer<Over>>,
    race_select_spell_power_components: Query<&RaceSelectSpellPower>,
    spell_descriptor_tooltip: Single<
        (Entity, &mut Visibility, &Children),
        With<SpellDescriptionTooltip>,
    >,
    mut spell_descriptor_child_q: Query<(&mut ImageNode, &mut Text, &mut Node)>,
    mut name_node_q: Query<&mut Text, Without<ImageNode>>,
    esm_store: Res<EsmStore>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(race_select_spell_power_comp) = race_select_spell_power_components.get(event.entity)
    else {
        dbg!("Not found");
        return;
    };
    let Some(corresponding_spell) = esm_store.spells.get(&race_select_spell_power_comp.0) else {
        return;
    };

    let effects = &corresponding_spell.effect_list.list;
    let (spell_descriptor_entity, mut visibility, spell_tooltip_children) =
        spell_descriptor_tooltip.into_inner();
    *visibility = Visibility::Visible;

    // Update spell name
    if let Some(&name_child) = spell_tooltip_children.get(0) {
        if let Ok(mut text) = name_node_q.get_mut(name_child) {
            text.0 = corresponding_spell.name.clone();
        }
    }
    for (index, indexed_enam_struct) in corresponding_spell.effect_list.list.iter().enumerate() {
        let Some(effect) = esm_store
            .magic_effects
            .get(&indexed_enam_struct.data.effect_id)
        else {
            continue;
        };
        let effect_path_cleaned = clean_path_common(&effect.icon);
        let full_effect_path = format!("data/Data Files/Icons/{}", effect_path_cleaned);
        let desc = get_magic_effect_string(
            effect,
            &indexed_enam_struct.data.attribute,
            &indexed_enam_struct.data.skill,
            &esm_store,
        );

        if check_exists(&full_effect_path) {
            let effect_image = asset_server.load(full_effect_path);
            // ui index is +1 because 0 is the spell name, the rest are the actual spells
            let ui_index = index + 1;
            if let Some(&child) = spell_tooltip_children.get(ui_index) {
                if let Ok((mut image_node, mut text, mut node)) =
                    spell_descriptor_child_q.get_mut(child)
                {
                    image_node.image = effect_image;
                    text.0 = desc;
                    node.display = Display::DEFAULT;
                }
            }
            // Spawn new children, since we didn't have enough to display all the effects
            else {
                let new_descriptor_entity = commands
                    .spawn((Node::default(), ImageNode::default(), Text::new(desc)))
                    .id();
                commands
                    .entity(spell_descriptor_entity)
                    .add_child(new_descriptor_entity);
            }
        }
    }
    // --- 3. HIDE EXTRAS ---
    // Start skipping at effects.len() + 1 to account for the Name child
    let total_active_count = effects.len() + 1;
    if spell_tooltip_children.len() > total_active_count {
        for &child in &spell_tooltip_children[total_active_count..] {
            // Check both queries since we don't know if the extra was a name or effect
            if let Ok((_, _, mut node)) = spell_descriptor_child_q.get_mut(child) {
                node.display = Display::None;
            }
        }
    }
    dbg!(&esm_store.magic_effects.get(&race_select_spell_power_comp.0));
}
pub fn update_tooltip_position<const OFFSET: i32>(
    event: On<Pointer<Move>>,
    mut tooltip: Single<&mut Node, With<SpellDescriptionTooltip>>,
) {
    dbg!("updating position");
    tooltip.top = Val::Px(event.pointer_location.position.y + OFFSET as f32);
    tooltip.left = Val::Px(event.pointer_location.position.x);
}
pub fn on_out_hide_tooltip(
    _event: On<Pointer<Out>>,
    mut tooltip_visibility: Single<&mut Visibility, With<SpellDescriptionTooltip>>,
) {
    **tooltip_visibility = Visibility::Hidden;
}
pub fn get_magic_effect_string(
    effect: &MagicEffect,
    attribute_refid: &RefId,
    skill_refid: &RefId,
    esm_store: &EsmStore,
) -> String {
    // 1. Determine the GMST key based on the effect ID
    let gmst_key = match &effect.id {
        id if *id == MagicEffect::ABSORBATTRIBUTE || *id == MagicEffect::ABSORBSKILL => {
            Some("sAbsorb")
        }
        id if *id == MagicEffect::DAMAGEATTRIBUTE || *id == MagicEffect::DAMAGESKILL => {
            Some("sDamage")
        }
        id if *id == MagicEffect::DRAINATTRIBUTE || *id == MagicEffect::DRAINSKILL => {
            Some("sDrain")
        }
        id if *id == MagicEffect::FORTIFYATTRIBUTE || *id == MagicEffect::FORTIFYSKILL => {
            Some("sFortify")
        }
        id if *id == MagicEffect::RESTOREATTRIBUTE || *id == MagicEffect::RESTORESKILL => {
            Some("sRestore")
        }
        _ => None,
    };
    let skill = esm_store.skills.get(skill_refid);
    let gmsts = &esm_store.gamesettings;

    // 2. Lookup the prefix string (O(1) search)
    let mut spell_line: String = gmst_key
        .and_then(|key| gmsts.search(key))
        .and_then(|setting| setting.value.0.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| effect.name.clone());
    // 3. Append target name using bitflag .contains()
    let flags = effect.magic_effect_data.flags;

    if flags.contains(MagicEffectFlags::TARGET_SKILL) {
        dbg!("contains skill");
        if let Some(s) = skill {
            dbg!("contains skill");
            spell_line.push(' ');
            spell_line.push_str(&s.name);
        }
    } else if flags.contains(MagicEffectFlags::TARGET_ATTRIBUTE) {
        spell_line.push(' ');
        spell_line.push_str(&attribute_refid.resolve_string());
    }

    spell_line
}
