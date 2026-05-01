// --- Animation Name Resolution Helpers ---

use bevy_nif::nif_animation::NifAnimator;
use common::{ActorState, CharacterState};

/// Generates a prioritized list of animation names based on a base name and weapon type.
/// e.g., ("walkforward", weapon_type) -> ["walkforwardaxe", "walkforward1h", "walkforward"]
fn generate_weapon_anim_candidates(base_group: &str, weapon_type: i32) -> Vec<String> {
    let (specific_suffix, fallback_suffix) = get_weapon_suffixes(weapon_type);
    let mut candidates = Vec::with_capacity(3);

    if let Some(suffix) = specific_suffix {
        candidates.push(format!("{}{}", base_group, suffix));
    }
    if let Some(suffix) = fallback_suffix {
        if specific_suffix != fallback_suffix {
            // Avoid duplicates
            candidates.push(format!("{}{}", base_group, suffix));
        }
    }
    candidates.push(base_group.to_string());
    candidates
}

/// Finds the first valid animation name from a list of candidates.
fn find_first_valid_animation(candidates: &[String], animator: &NifAnimator) -> Option<String> {
    candidates
        .iter()
        .find(|name| animator.animation_definitions.contains_key(*name))
        .cloned()
}

/// Resolves a logical idle state into a final, playable animation group name.
pub fn resolve_idle_anim_name(
    state: CharacterState,
    actor_state: &ActorState,
    animator: &NifAnimator,
) -> Option<String> {
    let base_group = match state {
        CharacterState::Idle => "idle",
        CharacterState::IdleSneak => "idlesneak",
        CharacterState::IdleSwim => "idleswim",
        _ => return None, // Not an idle state
    };
    resolve_anim_name(base_group, actor_state.combat.weapon_type, animator)
}

/// Resolves the JUMP INTRO animation group name (e.g., "jumphh").
pub fn resolve_jump_intro_name(weapon_type: i32, animator: &NifAnimator) -> Option<String> {
    resolve_anim_name("jump", weapon_type, animator)
}

/// Resolves the JUMP LANDING animation group name (e.g., "jumphh_outro").
pub fn resolve_jump_landing_name(weapon_type: i32, animator: &NifAnimator) -> Option<String> {
    // For landing, we check variations of "jump_outro"
    let base_outro = "jump_outro";
    if let Some(name) = resolve_anim_name("jump", weapon_type, animator) {
        let outro_name = format!("{}_outro", name);
        if animator.animation_definitions.contains_key(&outro_name) {
            return Some(outro_name);
        }
    }
    // Fallback for base "jump_outro" if no weapon-specific one is found
    if animator.animation_definitions.contains_key(base_outro) {
        Some(base_outro.to_string())
    } else {
        None
    }
}
/// Resolves a logical movement state into a final, playable animation group name,
/// including handling run-to-walk and swim-to-walk fallbacks.
pub fn resolve_movement_anim_name(
    state: CharacterState,
    actor_state: &ActorState,
    animator: &NifAnimator,
) -> Option<String> {
    // The state is WeaponEquipped, not Equipping or Unequipping.
    // let weapon_equipped = actor_state.combat.weapon_type > 0;
    let mut base_group = match state {
        CharacterState::WalkForward => "walkforward",
        CharacterState::WalkBack => "walkback",
        CharacterState::WalkLeft => "walkleft",
        CharacterState::WalkRight => "walkright",
        CharacterState::SwimWalkForward => "swimwalkforward",
        CharacterState::SwimWalkBack => "swimwalkback",
        CharacterState::SwimWalkLeft => "swimwalkleft",
        CharacterState::SwimWalkRight => "swimwalkright",
        CharacterState::RunForward => "runforward",
        CharacterState::RunBack => "runback",
        CharacterState::RunLeft => "runleft",
        CharacterState::RunRight => "runright",
        CharacterState::SwimRunForward => "swimrunforward",
        CharacterState::SwimRunBack => "swimrunback",
        CharacterState::SwimRunLeft => "swimrunleft",
        CharacterState::SwimRunRight => "swimrunright",
        CharacterState::SneakForward => "sneakforward",
        CharacterState::SneakBack => "sneakback",
        CharacterState::SneakLeft => "sneakleft",
        CharacterState::SneakRight => "sneakright",
        CharacterState::TurnLeft => "turnleft",
        CharacterState::TurnRight => "turnright",
        CharacterState::SwimTurnLeft => "swimturnleft",
        CharacterState::SwimTurnRight => "swimturnright",
        _ => return None,
    };
    // Use the generic resolver first. If it finds an animation (like "runforward1h"), return it immediately.
    if let Some(found) = resolve_anim_name(base_group, actor_state.combat.weapon_type, animator) {
        return Some(found);
    }

    // --- Fallback Logic ---
    // Fallback for swim animations (try non-swim version if specific swim anim not found)
    if base_group.starts_with("swim")
        && !animator
            .animation_definitions
            .keys()
            .any(|k| k.starts_with(base_group))
    {
        base_group = base_group.strip_prefix("swim").unwrap_or(base_group);
    }

    // Fallback for run animations (try walk version if run anim not found)
    if base_group.starts_with("run") {
        if find_first_valid_animation(&[base_group.to_string()], animator).is_none() {
            if let Some(walk_base) = base_group.strip_prefix("run") {
                let walk_group = format!("walk{}", walk_base);
                return Some(walk_group);
            }
        }
    }

    Some(base_group.to_string())
}
/// Converts a running state to its walking equivalent.
pub fn run_state_to_walk_state(state: CharacterState) -> CharacterState {
    match state {
        CharacterState::RunForward => CharacterState::WalkForward,
        CharacterState::RunBack => CharacterState::WalkBack,
        CharacterState::RunLeft => CharacterState::WalkLeft,
        CharacterState::RunRight => CharacterState::WalkRight,
        CharacterState::SwimRunForward => CharacterState::SwimWalkForward,
        CharacterState::SwimRunBack => CharacterState::SwimWalkBack,
        CharacterState::SwimRunLeft => CharacterState::SwimWalkLeft,
        CharacterState::SwimRunRight => CharacterState::SwimWalkRight,
        _ => state, // Not a running state, return as-is.
    }
}
/// A helper to get the correct follow-through suffix based on attack strength.
pub fn get_follow_suffix(attack_strength: f32) -> &'static str {
    if attack_strength < 0.33 {
        "_small_follow"
    } else if attack_strength < 0.66 {
        "_medium_follow"
    } else {
        "_large_follow"
    }
}

// Corresponds to the weapon's main animation group (e.g., "weapontwohand", "bow")
pub fn get_weapon_long_group(weapon_type: i32) -> &'static str {
    // Based on ESM::Weapon::Type
    match weapon_type {
        0 => "handtohand",
        1..=4 => "weapononehand",
        5..=7 => "weapontwohand", // For swords, axes, blunts
        8 => "weapontwowide",     // Spears use a different group
        9 => "bow",
        10 | 13 => "crossbow",
        11 => "throwweapon", // Corrected from "throwing" to match anim names
        12 => "spellcast",   // Corrected from "spell" to match anim names
        _ => "",
    }
}

/// Returns the specific and fallback animation suffixes for a given weapon type.
fn get_weapon_suffixes(weapon_type: i32) -> (Option<&'static str>, Option<&'static str>) {
    // These now match the suffixes found in your animation list (hh, 1h, 2c, 2w, etc.)
    match weapon_type {
        0 => (Some("hh"), None),     // Hand-to-hand
        1..=4 => (Some("1h"), None), // All 1-handed weapons use "1h"
        5..=7 => (Some("2c"), None), // 2-handed close-stance weapons (swords, axes)
        8 => (Some("2w"), None),     // 2-handed wide-stance weapons (spears)
        9 => (Some("bow"), None),
        10 | 13 => (Some("crossbow"), Some("1h")), // Crossbows use 1h for movement
        12 => (Some("spell"), None),               // Spellcasting uses "spell"
        _ => (None, None),                         // Thrown, Spell, and None have no suffix
    }
}
/// A generic function to find the best available animation name.
/// It searches for weapon-specific versions before falling back to the base name.
fn resolve_anim_name(base_group: &str, weapon_type: i32, animator: &NifAnimator) -> Option<String> {
    // Only search for weapon-specific versions if a weapon is equipped.
    if weapon_type > 0 {
        let (specific_suffix, fallback_suffix) = get_weapon_suffixes(weapon_type);
        let mut candidates = Vec::with_capacity(3);

        if let Some(suffix) = specific_suffix {
            candidates.push(format!("{}{}", base_group, suffix));
        }
        if let Some(suffix) = fallback_suffix {
            // Avoid duplicates if specific and fallback are the same
            if specific_suffix != fallback_suffix {
                candidates.push(format!("{}{}", base_group, suffix));
            }
        }
        candidates.push(base_group.to_string());

        // Find the first valid candidate
        if let Some(found) = candidates
            .iter()
            .find(|name| animator.animation_definitions.contains_key(*name))
        {
            return Some(found.clone());
        }
    }

    // If no weapon or no specific animation was found, check if the base animation exists.
    if animator.animation_definitions.contains_key(base_group) {
        Some(base_group.to_string())
    } else {
        None
    }
}
