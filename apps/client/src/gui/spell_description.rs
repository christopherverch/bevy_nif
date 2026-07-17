use bevy::prelude::*;
use common::helpers::{check_exists, clean_path_common};
use esm_parser_lib::{
    RefId,
    types::{
        effect_list::ENAMStruct,
        enums::EffectRange,
        flags::MagicEffectFlags,
        gmst::GameSettingVariant,
        magic_effect::{MagicEffect, MagnitudeDisplayType},
        spell::{Spell, SpellType},
    },
};
use esm_store::EsmStore;

use crate::gui::{RenderPosition, race_select::RaceSelectSpellPower};

#[derive(Component)]
pub struct SpellDescriptionTooltip;
pub fn setup_spell_description_tooltip(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(20.0),
                top: Val::Percent(10.0),
                flex_direction: FlexDirection::Column,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Start,
                justify_content: JustifyContent::Start,
                ..default()
            },
            ZIndex(RenderPosition::Tooltip.as_i32()),
            Pickable::default(),
            SpellDescriptionTooltip,
            BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 1.0)),
        ))
        .with_children(|spell_tooltip_panel| {
            // Spell name
            spell_tooltip_panel.spawn((
                Node {
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
            dbg!(corresponding_spell);
        }
    }
    for child in spell_tooltip_children.iter().skip(1) {
        commands.entity(child).despawn();
    }
    for indexed_enam_struct in effects.iter() {
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
        dbg!(effect);
        let spell_effect_formatted = format_spell_effect_line(
            corresponding_spell,
            effect,
            &indexed_enam_struct.data,
            &esm_store,
        );

        if check_exists(&full_effect_path) {
            let effect_image = asset_server.load(full_effect_path);
            // ui index is +1 because 0 is the spell name, the rest are the actual spells
            // Spawn a container that holds the image and text
            let effect_row = commands
                .spawn((Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center, // Vertically centers the text and image
                    column_gap: Val::Px(8.0),        // Adds space between icon and text
                    ..default()
                },))
                .id();
            let icon = commands
                .spawn((
                    ImageNode::new(effect_image),
                    Node {
                        width: Val::Px(32.0),
                        height: Val::Px(32.0),
                        ..default()
                    }, // Set fixed size for icon
                ))
                .id();
            let text = commands.spawn(Text::new(spell_effect_formatted)).id();

            commands.entity(effect_row).add_children(&[icon, text]);
            commands
                .entity(spell_descriptor_entity)
                .add_child(effect_row);
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

    // 2. Lookup the prefix string
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
fn format_effect_string(data: &ENAMStruct, effect: &MagicEffect, esm_store: &EsmStore) {
    if data.duration > 0 {
        if data.min_magnitude == data.max_magnitude {}
    }

    let effect_name = get_magic_effect_string(effect, &data.attribute, &data.skill, &esm_store);
}
pub fn format_spell_effect_line(
    spell: &Spell,
    effect: &MagicEffect,
    data: &ENAMStruct,
    esm_store: &EsmStore,
) -> String {
    // 1. Start with the base effect name and targeting (e.g., "Restore Health on Self")
    let mut spell_line = get_magic_effect_string(effect, &data.attribute, &data.skill, esm_store);

    // Helper to fetch localized strings
    let get_gmst = |key: &str| {
        let gmst_opt = esm_store.gamesettings.search(key);
        if let Some(gmst) = gmst_opt {
            return gmst.as_str().unwrap_or("");
        }
        ""
    };

    // 2. Magnitude Formatting
    if (data.min_magnitude != 0 || data.max_magnitude != 0)
        && !effect
            .magic_effect_data
            .flags
            .contains(MagicEffectFlags::NO_MAGNITUDE)
    {
        let display_type = effect.get_magnitude_display_type();

        match display_type {
            MagnitudeDisplayType::TimesInt => {
                let x_times = get_gmst("sXTimesINT");
                if data.min_magnitude == data.max_magnitude {
                    spell_line.push_str(&format!(
                        " {:.1}{}",
                        data.min_magnitude as f32 / 10.0,
                        x_times
                    ));
                } else {
                    let to = get_gmst("sTo");
                    spell_line.push_str(&format!(
                        " {:.1} {} {:.1}{}",
                        data.min_magnitude as f32 / 10.0,
                        to,
                        data.max_magnitude as f32 / 10.0,
                        x_times
                    ));
                }
            }
            MagnitudeDisplayType::None => {}
            _ => {
                // Percentage, Points, Feet, Level
                spell_line.push_str(&format!(" {}", data.min_magnitude));
                if data.min_magnitude != data.max_magnitude {
                    spell_line.push_str(&format!(" {} {}", get_gmst("sTo"), data.max_magnitude));
                }

                let suffix = match display_type {
                    MagnitudeDisplayType::Percentage => get_gmst("spercent").to_string(),
                    MagnitudeDisplayType::Feet => format!(" {}", get_gmst("sfeet")),
                    MagnitudeDisplayType::Level => format!(
                        " {}",
                        if data.min_magnitude == data.max_magnitude && data.min_magnitude.abs() == 1
                        {
                            get_gmst("sLevel")
                        } else {
                            get_gmst("sLevels")
                        }
                    ),
                    _ => format!(
                        " {}",
                        if data.min_magnitude == data.max_magnitude && data.min_magnitude.abs() == 1
                        {
                            get_gmst("spoint")
                        } else {
                            get_gmst("spoints")
                        }
                    ),
                };
                spell_line.push_str(&suffix);
            }
        }
    }

    // 3. Duration and Area (only if not constant)
    if !(spell.data.spell_type == SpellType::Ability) {
        // Duration
        if data.duration > 0
            && !effect
                .magic_effect_data
                .flags
                .contains(MagicEffectFlags::NO_DURATION)
        {
            let s_for = get_gmst("sfor");
            let sec = if data.duration == 1 {
                get_gmst("ssecond")
            } else {
                get_gmst("sseconds")
            };
            spell_line.push_str(&format!(" {} {} {}", s_for, data.duration, sec));
        }

        // Area
        if data.area > 0 {
            spell_line.push_str(&format!(
                " {} {} {}",
                get_gmst("sin"),
                data.area,
                get_gmst("sfootarea")
            ));
        }
        //TODO::
        let no_target = false;
        // Range
        if !no_target {
            let range_str = match data.range {
                EffectRange::OnSelf => get_gmst("sRangeSelf"),
                EffectRange::OnTouch => get_gmst("sRangeTouch"),
                EffectRange::OnTarget => get_gmst("sRangeTarget"),
            };
            spell_line.push_str(&format!(" {} {}", get_gmst("sonword"), range_str));
        }
    }

    spell_line
}
