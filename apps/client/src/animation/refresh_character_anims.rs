use bevy_nif::nif_animation::{BlendMask, NifAnimator};
use common::mechanics::{CharacterController, CharacterState};
use esm_parser_lib::types::enums::WeaponType;

use crate::animation::name_resolver::{
    fallback_short_weapon_group, get_weapon_short_group, is_real_weapon,
    movement_state_to_anim_group,
};

fn refresh_movement_anims(
    character_controller: &mut CharacterController,
    state: CharacterState,
    force: bool,
    is_biped: bool,
    animator: &NifAnimator,
) {
    // Same as previous and not force update
    if state == character_controller.movement.state && !force {
        return;
    }
    let movement_anim_group = movement_state_to_anim_group(state);
    if movement_anim_group.is_empty() {
        if !character_controller.movement.current_group.is_empty() {
            //reset_current_idle_state
        }
        //reset_current_movement_state();
        return;
    }
    // Logic to handle the "swim" fallback
    let mut movement_anim_name = movement_anim_group.to_string();
    let mut swimpos = movement_anim_name.find("swim");
    // If Animation doesn't exist, check if it was a swim animation
    if !animator
        .animation_definitions
        .contains_key(&movement_anim_name)
    {
        // Was a swim animation, remove "swim" because the swim version didn't exist
        if let Some(pos) = swimpos {
            movement_anim_name.replace_range(pos..pos + 4, "");
            swimpos = None; // No longer a swim animation
        }
    }
    // 4. Weapon Suffix Logic (only if it wasn't a swim animation)
    let mut move_mask = BlendMask::ALL;
    let mut weap_short_group =
        get_weapon_short_group(character_controller.combat.weapon_type, is_biped);
    // Non-biped creatures don't use spellcasting-specific movement animations.
    if !is_real_weapon(character_controller.combat.weapon_type) && !is_biped {
        weap_short_group = "";
    }

    // Not a swim animation and has a weapon short group
    if swimpos.is_none() && !weap_short_group.is_empty() {
        // Spellcasting stance turning is a special case
        let weap_movement_anim_name = if character_controller.combat.weapon_type
            == WeaponType::Spell
            && is_turning(character_controller)
        {
            format!("{}{}", weap_short_group, movement_anim_name)
        } else {
            format!("{}{}", movement_anim_name, weap_short_group)
        };

        if animator
            .animation_definitions
            .contains_key(&weap_movement_anim_name)
        {
            movement_anim_name = weap_movement_anim_name;
        } else {
            movement_anim_name = fallback_short_weapon_group(
                character_controller.combat.weapon_type,
                movement_anim_name,
                Some(&mut move_mask),
                animator,
                is_biped,
            );
        }
    }
    // If this anim doesn't exist, try removing "run"
    if !animator
        .animation_definitions
        .contains_key(&movement_anim_name)
    {
        let run_pos = movement_anim_name.find("run");
        if let Some(pos) = run_pos {
            movement_anim_name.replace_range(pos..pos + 3, "");
        }
        // Still can't find the animation, just reset our state
        if !animator
            .animation_definitions
            .contains_key(&movement_anim_name)
        {
            if !character_controller.movement.current_group.is_empty() {
                //reset_current_idle_state();
            }
            //reset_current_movement_state();
            return;
        }
    }
    let mut start_point = 0.0;
    // If we're playing the same animation, start it from the point it ended

    if !character_controller.movement.current_group.is_empty()
        && movement_anim_name == character_controller.movement.current_group
    {}
    character_controller.movement.animation_has_movement = true;
}
fn is_turning(character_controller: &CharacterController) -> bool {
    return character_controller.movement.state == CharacterState::TurnLeft
        || character_controller.movement.state == CharacterState::TurnRight
        || character_controller.movement.state == CharacterState::SwimTurnLeft
        || character_controller.movement.state == CharacterState::SwimTurnLeft;
}
