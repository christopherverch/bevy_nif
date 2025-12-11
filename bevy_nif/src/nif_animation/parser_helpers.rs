// src/nif_animation/parser_helpers.rs

use bevy::log::warn;
use bevy::math::Vec3;
use bevy_animation::animatable::Animatable;
use bevy_animation::animation_curves::AnimatableKeyframeCurve;

use super::bevy_types::{
    REGION_INDEX_LEFT_ARM, REGION_INDEX_LOWER_BODY, REGION_INDEX_RIGHT_ARM, REGION_INDEX_TORSO,
    REGION_ROOT_LEFT_ARM, REGION_ROOT_LOWER_BODY, REGION_ROOT_RIGHT_ARM, REGION_ROOT_TORSO,
};
use crate::skeleton::Skeleton;

// --- Bone Region Determination Helper ---
// (Moved here as it's a parser/skeleton interpretation helper)

/// Helper to determine the primary discrete region index for a bone.
pub(super) fn determine_bone_primary_region_index(bone_name: &str, skeleton: &Skeleton) -> usize {
    if skeleton.is_descendant_of_or_is(bone_name, REGION_ROOT_LEFT_ARM) {
        return REGION_INDEX_LEFT_ARM;
    }
    if skeleton.is_descendant_of_or_is(bone_name, REGION_ROOT_RIGHT_ARM) {
        return REGION_INDEX_RIGHT_ARM;
    }
    if skeleton.is_descendant_of_or_is(bone_name, REGION_ROOT_TORSO) {
        return REGION_INDEX_TORSO;
    }
    if skeleton.is_descendant_of_or_is(bone_name, REGION_ROOT_LOWER_BODY) {
        return REGION_INDEX_LOWER_BODY;
    }
    if skeleton.get_bone_by_name(bone_name).is_some() {
        // Default for unclassified bones that are part of the skeleton
        return REGION_INDEX_LOWER_BODY;
    }
    warn!(
        "Bone '{}' could not be classified into a primary region. Defaulting to Lower Body (Index {}).",
        bone_name, REGION_INDEX_LOWER_BODY
    );
    REGION_INDEX_LOWER_BODY
}

pub fn make_bevy_curve<T: Animatable + Copy>(
    keyframes: &[(f32, T)],
) -> Option<AnimatableKeyframeCurve<T>> {
    // AnimatableKeyframeCurve::new fails if there are < 2 keyframes.
    match AnimatableKeyframeCurve::new(keyframes.iter().copied()) {
        Ok(curve) => Some(curve),
        Err(_) => {
            // If creation failed, it's likely due to having only 0 or 1 keyframes.
            // If there's exactly one keyframe, we can create a "constant" curve
            // by duplicating that keyframe.
            if let Some(first_key) = keyframes.first() {
                let constant_curve_keys = vec![*first_key, *first_key];
                // This second attempt should not fail.
                AnimatableKeyframeCurve::new(constant_curve_keys).ok()
            } else {
                // No keyframes, no curve.
                None
            }
        }
    }
}
/// Filters and re-times keyframes from a controller to fit a new clip's time range.
pub fn filter_and_retime_keyframes<'a, T: Copy + 'a>(
    keys_iter: impl Iterator<Item = (f32, T)> + 'a,
    clip_start: f32,
    clip_end: f32,
) -> Vec<(f32, T)> {
    if clip_end <= clip_start {
        return Vec::new();
    }

    // Collect the iterator into a single Vec inside the function.
    // This allows us to perform the multiple searches needed below (rfind, filter, find)
    let all_keys: Vec<(f32, T)> = keys_iter.collect();

    let mut new_keys = Vec::new();

    // Find the last keyframe *before* or at the start of our clip to set the initial value.
    if let Some(key_before) = all_keys.iter().rfind(|(t, _)| *t <= clip_start + 1e-4) {
        new_keys.push((0.0, key_before.1));
    }

    // Add all keyframes that fall strictly within the clip's time range, retimed relative to clip_start.
    for (time, value) in all_keys
        .iter()
        .filter(|(t, _)| *t > clip_start && *t < clip_end)
    {
        new_keys.push((*time - clip_start, *value));
    }

    // Find the first keyframe *after* or at the end of our clip to set the final value.
    if let Some(key_after) = all_keys.iter().find(|(t, _)| *t >= clip_end - 1e-4) {
        // Ensure the last keyframe is exactly at the new clip's duration.
        if new_keys
            .last()
            .map_or(true, |(t, _)| *t < (clip_end - clip_start) - 1e-4)
        {
            new_keys.push((clip_end - clip_start, key_after.1));
        }
    }

    // If we have no keys but there was one before the clip, hold that value for the duration.
    if new_keys.len() < 2 && !new_keys.is_empty() {
        if let Some(first) = new_keys.first().cloned() {
            new_keys.push((clip_end - clip_start, first.1));
        }
    }

    new_keys
}
pub fn is_inherently_looping(name: &str) -> bool {
    // This list mirrors `MWMechanics::getAllWeaponTypeShortGroups()` and is used to strip suffixes.
    const WEAPON_SUFFIXES: &[&str] = &[
        "crossbow", // Must be checked before "bow"
        "onehand", "twohand", "twowide", "h2h", "1h", "2h", "2w", "bow", "spell", "thrown",
    ];

    let mut base_name = name;
    let mut longest_suffix_len = 0;

    // Find the longest matching suffix from the list and strip it.
    // E.g., for "runforwardonehand", this will strip "onehand", leaving "runforward".
    for suffix in WEAPON_SUFFIXES {
        if name.ends_with(suffix) && suffix.len() > longest_suffix_len {
            longest_suffix_len = suffix.len();
        }
    }

    if longest_suffix_len > 0 {
        base_name = &name[..name.len() - longest_suffix_len];
    }

    // This match statement mirrors the `loopingAnimations` set in OpenMW's source code.
    matches!(
        base_name,
        "walkforward"
            | "walkback"
            | "walkleft"
            | "walkright"
            | "swimwalkforward"
            | "swimwalkback"
            | "swimwalkleft"
            | "swimwalkright"
            | "runforward"
            | "runback"
            | "runleft"
            | "runright"
            | "swimrunforward"
            | "swimrunback"
            | "swimrunleft"
            | "swimrunright"
            | "sneakforward"
            | "sneakback"
            | "sneakleft"
            | "sneakright"
            | "turnleft"
            | "turnright"
            | "swimturnleft"
            | "swimturnright"
            | "spellturnleft"
            | "spellturnright"
            | "torch"
            | "idle"
            | "idle2"
            | "idle3"
            | "idle4"
            | "idle5"
            | "idle6"
            | "idle7"
            | "idle8"
            | "idle9"
            | "idlesneak"
            | "idlestorm"
            | "idleswim"
            | "jump"
            | "inventoryhandtohand"
            | "inventoryweapononehand"
            | "inventoryweapontwohand"
            | "inventoryweapontwowide"
    )
}
/// Samples a `Vec3` animation curve at a specific absolute time.
pub fn sample_vec3_curve(keys: &[(f32, Vec3)], time: f32) -> Option<Vec3> {
    if keys.is_empty() {
        return None;
    }

    let index = keys.partition_point(|(k_time, _)| *k_time < time);

    match index {
        0 => Some(keys[0].1),
        i if i >= keys.len() => Some(keys.last().unwrap().1),
        _ => {
            let (prev_time, prev_val) = keys[index - 1];
            let (next_time, next_val) = keys[index];
            let dt = next_time - prev_time;
            if dt <= 0.0 {
                return Some(prev_val);
            }
            let t = (time - prev_time) / dt;
            Some(prev_val.lerp(next_val, t))
        }
    }
}
