// src/nif_animation/text_key_parser.rs

use bevy::prelude::{Vec3, error, info, warn};
use std::collections::HashMap;

// NIF types from your parser crate
use crate::nif::types::{NiKeyframeController, ParsedBlock, ParsedNifData, TextKey as NifTextKey};

// Helpers from our new module structure
use super::parser_helpers::{
    KNOWN_GENERIC_EVENT_GROUP_NAMES, parse_nif_text_key_value, to_bevy_quat,
};
// Intermediate types this parser produces
use super::intermediate_types::{AnimationSequence, BoneAnimationCurve, TextKeyEvent};
// Bevy types for constants if needed
use super::bevy_types::REGION_ROOT_LOWER_BODY;

#[derive(Debug, Clone)]
struct DefinedAnimationSegment {
    original_name: String,
    lower_name: String,
    abs_start_time: f32,
    abs_stop_time: f32,
}

#[derive(Debug, Clone)]
struct AnimationBoundaryMarker {
    original_group_name: String,
    lower_group_name: String,
    time: f32,
    is_start_event: bool,
}

/// Main function to extract all AnimationSequences from NIF text keys.
pub fn extract_animation_sequences_from_text_keys(
    nif_data: &ParsedNifData,
    all_bone_controllers: &HashMap<usize, Vec<&NiKeyframeController>>,
    global_nif_text_keys: &[NifTextKey],
) -> Result<HashMap<String, AnimationSequence>, String> {
    let mut final_animation_sequences: HashMap<String, AnimationSequence> = HashMap::new();
    if global_nif_text_keys.is_empty() {
        return Ok(final_animation_sequences);
    }

    // --- Pass 1: Collect all Start boundary markers to identify potential animation groups ---
    let mut start_markers: Vec<AnimationBoundaryMarker> = Vec::new();
    for nif_key in global_nif_text_keys {
        for line in nif_key.value.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Some((original_group, lower_group, lower_cmd)) = parse_nif_text_key_value(line) {
                if KNOWN_GENERIC_EVENT_GROUP_NAMES.contains(&lower_group.as_str()) {
                    continue;
                }
                if original_group.is_empty() {
                    continue;
                }

                if lower_cmd == "start" {
                    start_markers.push(AnimationBoundaryMarker {
                        original_group_name: original_group,
                        lower_group_name: lower_group,
                        time: nif_key.time,
                        is_start_event: true,
                    });
                }
            }
        }
    }
    start_markers.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    start_markers.dedup_by_key(|m| (m.lower_group_name.clone(), m.time.to_bits()));

    // --- Pass 2: For each start marker, find the full extent of its animation group ---
    let mut defined_segments: Vec<DefinedAnimationSegment> = Vec::new();
    for start_marker in &start_markers {
        let mut latest_time = start_marker.time;

        // The true end of an animation group is the timestamp of the last key of any kind
        // that belongs to that group or its subgroups.
        for nif_key in global_nif_text_keys {
            // Optimization: only check keys that occur at or after the start time
            if nif_key.time < start_marker.time {
                continue;
            }

            for line in nif_key.value.lines() {
                if let Some((_, parsed_lower_group, _)) = parse_nif_text_key_value(line) {
                    // If the key's group name starts with our base group name, it's part of the sequence.
                    if parsed_lower_group.starts_with(&start_marker.lower_group_name) {
                        latest_time = latest_time.max(nif_key.time);
                    }
                }
            }
        }

        if latest_time > start_marker.time {
            defined_segments.push(DefinedAnimationSegment {
                original_name: start_marker.original_group_name.clone(),
                lower_name: start_marker.lower_group_name.clone(),
                abs_start_time: start_marker.time,
                abs_stop_time: latest_time,
            });
        }
    }

    // Sort segments and remove duplicates that might arise from multiple identical start keys
    defined_segments.sort_by(|a, b| a.abs_start_time.partial_cmp(&b.abs_start_time).unwrap());
    defined_segments.dedup_by_key(|s| (s.lower_name.clone(), s.abs_start_time.to_bits()));

    // --- Pass 3: Populate each AnimationSegmentDef into a full AnimationSequence ---
    for segment_def in defined_segments {
        match populate_animation_sequence_data(
            &segment_def,
            nif_data,
            all_bone_controllers,
            global_nif_text_keys,
        ) {
            Ok(Some(sequence)) => {
                let mut final_name = sequence.name.clone();
                let mut counter = 0;
                while final_animation_sequences.contains_key(&final_name) {
                    counter += 1;
                    final_name = format!("{}_{}", sequence.name, counter);
                }
                if counter > 0 {
                    warn!(
                        "Renaming animation collision for '{}' to '{}'",
                        sequence.name, final_name
                    );
                }
                let mut final_sequence = sequence;
                final_sequence.name = final_name.clone();
                final_animation_sequences.insert(final_name, final_sequence);
            }
            Ok(None) => { /* Sequence was empty and intentionally skipped */ }
            Err(e) => {
                error!(
                    "Error populating sequence {}: {}",
                    segment_def.original_name, e
                );
            }
        }
    }

    info!(
        "--- Final Extracted Animation Sequences ({}): ---",
        final_animation_sequences.len()
    );
    let mut sorted_names_for_log: Vec<_> = final_animation_sequences.keys().cloned().collect();
    sorted_names_for_log.sort();
    for name in sorted_names_for_log {
        if let Some(seq) = final_animation_sequences.get(&name) {
            info!(
                "    - \"{}\" (AbsStart: {:.3}, AbsStop: {:.3}, Events: {}, BoneCurves: {})",
                name,
                seq.abs_start_time,
                seq.abs_stop_time,
                seq.events.len(),
                seq.bone_curves.len()
            );
        }
    }
    Ok(final_animation_sequences)
}

/// Populates an AnimationSequence with events, loop times, and bone curves.
fn populate_animation_sequence_data(
    segment_def: &DefinedAnimationSegment,
    nif_data: &ParsedNifData,
    all_bone_controllers: &HashMap<usize, Vec<&NiKeyframeController>>,
    global_nif_text_keys: &[NifTextKey],
) -> Result<Option<AnimationSequence>, String> {
    let mut current_sequence = AnimationSequence {
        name: segment_def.original_name.clone(),
        abs_start_time: segment_def.abs_start_time,
        abs_stop_time: segment_def.abs_stop_time,
        bone_curves: Vec::new(),
        events: Vec::new(),
        loop_start_time: None,
        loop_stop_time: None,
        initial_position: Vec3::ZERO,
        is_startup_to_loop: false,
    };
    let mut loop_start_abs: Option<f32> = None;
    let mut loop_stop_abs: Option<f32> = None;

    for nif_key_global in global_nif_text_keys {
        let key_abs_time = nif_key_global.time;
        if key_abs_time < current_sequence.abs_start_time - 1e-4
            || key_abs_time > current_sequence.abs_stop_time + 1e-4
        {
            continue;
        }

        for key_original_value_line in nif_key_global.value.lines() {
            if key_original_value_line.trim().is_empty() {
                continue;
            }

            if let Some((parsed_original_group, parsed_lower_group, parsed_lower_command)) =
                parse_nif_text_key_value(key_original_value_line)
            {
                // The key belongs to this segment if its name is a sub-group of the segment's name.
                if parsed_lower_group.starts_with(&segment_def.lower_name) {
                    if parsed_lower_command == "loop start" {
                        loop_start_abs = Some(key_abs_time);
                    } else if parsed_lower_command == "loop stop" {
                        loop_stop_abs = Some(key_abs_time);
                    } else if parsed_lower_command != "start" && parsed_lower_command != "stop" {
                        // Any other command is an event.
                        current_sequence.events.push(TextKeyEvent {
                            time: key_abs_time - current_sequence.abs_start_time,
                            value: key_original_value_line.to_string(),
                        });
                    }
                } else if KNOWN_GENERIC_EVENT_GROUP_NAMES.contains(&parsed_lower_group.as_str()) {
                    // Also add generic events that fall within our time range.
                    current_sequence.events.push(TextKeyEvent {
                        time: key_abs_time - current_sequence.abs_start_time,
                        value: key_original_value_line.to_string(),
                    });
                }
            }
        }
    }
    current_sequence
        .events
        .sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());

    // --- Loop Time Validation ---
    let sequence_duration_relative =
        current_sequence.abs_stop_time - current_sequence.abs_start_time;

    if let Some(start_abs) = loop_start_abs {
        let rel_start = (start_abs - current_sequence.abs_start_time).max(0.0);
        current_sequence.loop_start_time = Some(rel_start.min(sequence_duration_relative));
    }
    if let Some(stop_abs) = loop_stop_abs {
        let rel_stop = (stop_abs - current_sequence.abs_start_time).max(0.0);
        current_sequence.loop_stop_time = Some(rel_stop.min(sequence_duration_relative));
    }

    if current_sequence.loop_start_time.is_some() && current_sequence.loop_stop_time.is_none() {
        current_sequence.loop_stop_time = Some(sequence_duration_relative);
    }
    if let (Some(start), Some(stop)) = (
        current_sequence.loop_start_time,
        current_sequence.loop_stop_time,
    ) {
        if stop < start {
            current_sequence.loop_stop_time = Some(start);
        }
    }

    // --- Populate Bone Curves ---
    populate_bone_curves_for_sequence_internal(
        &mut current_sequence,
        nif_data,
        all_bone_controllers,
    )?;

    if !current_sequence.bone_curves.is_empty() || !current_sequence.events.is_empty() {
        Ok(Some(current_sequence))
    } else {
        Ok(None)
    }
}

/// Populates `bone_curves` and `initial_position` for an `AnimationSequence`.
fn populate_bone_curves_for_sequence_internal(
    sequence: &mut AnimationSequence,
    nif_data: &ParsedNifData,
    all_bone_controllers: &HashMap<usize, Vec<&NiKeyframeController>>,
) -> Result<(), String> {
    let mut bip01_initial_pos_set_this_sequence = false;
    for (target_node_idx, controllers) in all_bone_controllers {
        let target_node_block = nif_data.blocks.get(*target_node_idx).ok_or_else(|| {
            format!(
                "PopulateCurves: Invalid target_node_idx {}",
                target_node_idx
            )
        })?;
        let bone_name_str = match target_node_block {
            ParsedBlock::Node(node) => node.av_base.net_base.name.as_str(),
            _ => continue,
        };
        let mut bone_curve = BoneAnimationCurve {
            target_bone_name: bone_name_str.to_string(),
            ..Default::default()
        };
        let is_bip01 = bone_name_str.eq_ignore_ascii_case(REGION_ROOT_LOWER_BODY);
        for kfc in &*controllers {
            if let Some(kfd_link) = kfc.keyframe_data {
                if let Some(ParsedBlock::KeyframeData(keyframe_data_nif)) =
                    nif_data.blocks.get(kfd_link)
                {
                    for key_quat in &keyframe_data_nif.quaternion_keys {
                        if key_quat.time >= sequence.abs_start_time
                            && key_quat.time <= sequence.abs_stop_time + 1e-4
                        {
                            bone_curve.rotations.push((
                                key_quat.time - sequence.abs_start_time,
                                to_bevy_quat(key_quat.value),
                            ));
                        }
                    }
                    if is_bip01 && !bip01_initial_pos_set_this_sequence {
                        if let Some(first_key) = keyframe_data_nif.translations.first() {
                            let mut initial_pos_candidate = first_key.value;
                            let mut initial_pos_time = first_key.time;
                            for tk in &keyframe_data_nif.translations {
                                if tk.time >= sequence.abs_start_time
                                    && tk.time <= sequence.abs_stop_time + 1e-4
                                {
                                    if tk.time < initial_pos_time
                                        || initial_pos_time < sequence.abs_start_time
                                    {
                                        initial_pos_candidate = tk.value;
                                        initial_pos_time = tk.time;
                                    }
                                }
                            }
                            sequence.initial_position = initial_pos_candidate;
                            bip01_initial_pos_set_this_sequence = true;
                        }
                    }
                    for key_vec3 in &keyframe_data_nif.translations {
                        if key_vec3.time >= sequence.abs_start_time
                            && key_vec3.time <= sequence.abs_stop_time + 1e-4
                        {
                            if is_bip01 {
                                let translation = key_vec3.value;
                                bone_curve.translations.push((
                                    key_vec3.time - sequence.abs_start_time,
                                    Vec3::new(
                                        sequence.initial_position.x,
                                        sequence.initial_position.y,
                                        translation.z,
                                    ),
                                ));
                            } else {
                                bone_curve.translations.push((
                                    key_vec3.time - sequence.abs_start_time,
                                    key_vec3.value,
                                ));
                            }
                        }
                    }
                    if is_bip01 && !bip01_initial_pos_set_this_sequence {
                        if let Some(first_key) = keyframe_data_nif.translations.first() {
                            sequence.initial_position = first_key.value;
                        }
                    }
                    for key_float in &keyframe_data_nif.scales {
                        if key_float.time >= sequence.abs_start_time
                            && key_float.time <= sequence.abs_stop_time + 1e-4
                        {
                            bone_curve.scales.push((
                                key_float.time - sequence.abs_start_time,
                                Vec3::splat(key_float.value),
                            ));
                        }
                    }
                }
            }
        }
        if !bone_curve.rotations.is_empty()
            || !bone_curve.translations.is_empty()
            || !bone_curve.scales.is_empty()
        {
            bone_curve
                .rotations
                .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            bone_curve
                .translations
                .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            bone_curve
                .scales
                .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            sequence.bone_curves.push(bone_curve);
        }
    }
    Ok(())
}
