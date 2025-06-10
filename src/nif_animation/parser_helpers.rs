// src/nif_animation/parser_helpers.rs

use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::system::Query;
use bevy::log::warn;
use bevy::prelude::Quat;
use bevy_animation::animatable::Animatable;
use bevy_animation::animation_curves::AnimatableKeyframeCurve; // Assuming Bevy's math types

// Assuming your NIF parser output structs are in `crate::nif::types` or similar
// and your public Bevy-facing types (like region constants) are in `super::bevy_types`
use super::bevy_types::{
    REGION_INDEX_LEFT_ARM,
    REGION_INDEX_LOWER_BODY,
    REGION_INDEX_RIGHT_ARM,
    REGION_INDEX_TORSO,
    REGION_ROOT_LEFT_ARM,
    // Or wherever your REGION_ROOT constants are defined
    REGION_ROOT_LOWER_BODY,
    REGION_ROOT_RIGHT_ARM,
    REGION_ROOT_TORSO,
};
use crate::nif::skeleton::Skeleton;
use crate::nif::types::{
    NiKeyframeData,
    NiTextKeyExtraData,
    ParsedBlock,
    ParsedNifData,
    Quaternion as NifQuaternion, // Example: crate::nif::animation::Quaternion
    RecordLink,
    // Assuming your NIF parser defines Vector3 and Quaternion in crate::nif::types or crate::nif::base
    // and they are distinct from Bevy's. If they are type aliases to Bevy's, adjust accordingly.
};
use crate::nif_animation::bevy_types::{REGION_ROOT_LEFT_LEG, REGION_ROOT_RIGHT_LEG};

// --- Text Key Parsing Helper ---

/// Parses a NIF text key value string into its (OriginalGroupName, LowercaseGroupName, LowercaseCommand).
/// GroupName can contain colons. Command is typically the last part after the last colon.
/// Parses a NIF text key value string into its (OriginalGroupName, LowercaseGroupName, LowercaseCommand).
/// GroupName can contain colons. Command is typically one of the known_commands.
pub(super) fn parse_nif_text_key_value(line: &str) -> Option<(String, String, String)> {
    let line = line.trim();
    let parts: Vec<&str> = line.splitn(2, ':').collect();

    let (group, command_str) = if parts.len() == 2 {
        (parts[0].trim(), parts[1].trim())
    } else {
        ("", parts[0].trim()) // No group, the whole line is the command string
    };

    if command_str.is_empty() {
        return None;
    }

    let command_words: Vec<&str> = command_str.split_whitespace().collect();
    let command = command_words.last().cloned().unwrap_or("").to_lowercase();

    // The name part is all words in the command string *except* the last one.
    let name_part = if command_words.len() > 1 {
        command_words[0..command_words.len() - 1].join(" ")
    } else {
        // If there's only one word, it's the command, so there's no specific name part.
        // e.g., "start", "stop"
        String::new()
    };

    let full_original_name = if group.is_empty() {
        name_part
    } else if name_part.is_empty() {
        group.to_string()
    } else {
        format!("{}:{}", group, name_part)
    };

    if full_original_name.is_empty() {
        return None;
    }

    let full_lowercase_name = full_original_name.to_lowercase();

    Some((full_original_name, full_lowercase_name, command))
}

// Define known generic event prefixes (lowercase)
pub(super) const KNOWN_GENERIC_EVENT_GROUP_NAMES: [&str; 2] = ["soundgen", "sound"];

// --- NIF Data Access Helpers (from your test.txt / animation.rs_new.txt) ---

/// Resolves a RecordLink to a specific ParsedBlock and attempts to cast it using the provided caster function.
pub(super) fn get_block<'a, T>(
    nif_data: &'a ParsedNifData,
    link: RecordLink,
    caster: fn(&'a ParsedBlock) -> Option<&'a T>,
) -> Option<&'a T> {
    link.and_then(|index| nif_data.blocks.get(index).and_then(caster))
}

// Specific caster function for NiKeyframeData
pub(super) fn as_keyframe_data(block: &ParsedBlock) -> Option<&NiKeyframeData> {
    if let ParsedBlock::KeyframeData(kfd) = block {
        Some(kfd)
    } else {
        None
    }
}

// Specific caster function for NiTextKeyExtraData
pub(super) fn as_text_key_extra_data(block: &ParsedBlock) -> Option<&NiTextKeyExtraData> {
    if let ParsedBlock::TextKeyExtraData(tked) = block {
        Some(tked)
    } else {
        None
    }
}

// --- Coordinate/Type Conversion Helpers (from your test.txt / animation.rs_new.txt) ---

// Convert your NIF parser's Quaternion to Bevy's Quat
pub(super) fn to_bevy_quat(q_nif: NifQuaternion) -> Quat {
    // Assuming NifQuaternion is `pub type Quaternion = bevy::math::Quat;` or similar
    // If NifQuaternion is already Bevy's Quat, this function might just be `q_nif`
    // If it's a custom struct like `struct Quaternion { x:f32, y:f32, z:f32, w:f32 }` then:
    // Quat::from_xyzw(q_nif.x, q_nif.y, q_nif.z, q_nif.w)
    q_nif // Assuming it's already a Bevy Quat as per your types.txt
}

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
pub fn filter_and_retime_keyframes<T: Copy>(
    all_keys: &[(f32, T)],
    clip_start: f32,
    clip_end: f32,
) -> Vec<(f32, T)> {
    if clip_end <= clip_start {
        return Vec::new();
    }

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
} // Note: The functions `make_auto_or_constant_curve`, `filter_intro_track`,
// `filter_and_retime_loop_track`, and `split_animation_for_looping` are more
// involved in transforming `AnimationSequence` data into Bevy `AnimationClip`
// or modifying `AnimationSequence` itself. They could go into a separate
// `animation_utils.rs` or `bevy_animation_setup.rs` file, as they are less
// about raw NIF parsing and more about preparing data for the Bevy runtime.
// For simplicity now, if they are only used by the main extraction flow, they
// could also reside in `text_key_parser.rs` if that's where the primary
// extraction function will live.
// Let's assume for now that `split_animation_for_looping` will be called *after*
// the main extraction produces `AnimationSequence` objects.
