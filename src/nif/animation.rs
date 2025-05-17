use bevy::animation::AnimationClip;
use bevy::animation::AnimationTargetId;
use bevy::asset::Assets; // Keep Assets, Handle
use bevy::ecs::entity::Entity;
use bevy::math::curve::interval::InvalidIntervalError;
use bevy::prelude::*;
use bevy_animation::AnimationTarget;
use bevy_animation::animated_field;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

use crate::NiKeyframeController;
use crate::NiKeyframeData;
use crate::NiTextKeyExtraData;
use crate::ParsedBlock;
use crate::ParsedNifData;
use crate::RecordLink;
use crate::extra_data::ExtraFields;

use super::loader::Nif;
use super::skeleton::Skeleton;
use super::spawner::NeedsNifAnimator;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Resource)]
pub enum NifEventType {
    LoopAnimation {
        animation_name: String,
    },
    FreezeAnimation {
        animation_name: String,
    },
    ResumeIdle {
        animation_name: String,
    },

    /// Signals that a sound should be played.
    PlaySound {
        sound_name: String, // The identifier/name of the sound asset to play
    },
}
#[derive(Clone, Debug, Event, Serialize, Deserialize, Resource)]
pub struct NifEvent {
    pub skeleton_id: u64,
    pub event_type: NifEventType,
}

// --- Intermediate representation for an animation curve for a specific bone ---
#[derive(Default, Debug, Clone)]
pub struct BoneAnimationCurve {
    pub target_bone_name: String,                // Name of the NiNode (bone)
    pub rotations: Vec<(f32, bevy::math::Quat)>, // (time, rotation_value)
    pub translations: Vec<(f32, bevy::math::Vec3)>, // (time, translation_value)
    pub scales: Vec<(f32, f32)>, // (time, scale_value) - NIF usually has uniform scale
                                 // TODO: Add interpolation types if you plan to support more than linear/step
}
#[derive(Debug, Clone)]
pub struct TextKeyEvent {
    pub time: f32, // Time relative to the start of the AnimationSequence
    pub value: String,
}
// --- Intermediate representation for a full animation sequence ---
#[derive(Debug, Clone)]
pub struct AnimationSequence {
    pub name: String,
    pub abs_start_time: f32, // Absolute start time from NIF text keys
    pub abs_stop_time: f32,  // Absolute stop time from NIF text keys
    pub bone_curves: Vec<BoneAnimationCurve>,
    pub events: Vec<TextKeyEvent>,    // For soundgen, etc.
    pub loop_start_time: Option<f32>, // Relative to this sequence's start (0.0)
    pub loop_stop_time: Option<f32>,  // Relative to this sequence's start (0.0)
    pub initial_position: Vec3,
    pub is_startup_to_loop: bool,
}
// Temporary struct to hold data for a sequence being parsed
#[derive(Debug)]
struct ActiveSequenceData {
    name: String,
    abs_start_time: f32,
    // Stores all text keys (original value and absolute time) encountered
    // after "AnimName:start" and before or at "AnimName:stop".
    raw_text_keys_in_sequence: Vec<(f32, String)>,
}
#[derive(Resource, Debug, Default)]
pub struct SkeletonMap {
    pub root_skeleton_entity_map: HashMap<u64, Entity>,
    pub skeletons: HashMap<u64, Skeleton>,
}
#[derive(Component)]
pub struct NifAnimator {
    pub skeleton_id: u64,
    pub animations: HashMap<String, AnimationNodeIndex>,
    pub affected_bones: HashMap<AnimationNodeIndex, Vec<String>>,
    pub bone_masks: HashMap<String, u32>,
}

pub fn build_animation_clip_system(
    mut commands: Commands,
    skeleton_map_res: Res<SkeletonMap>,
    nif_assets: Res<Assets<Nif>>,
    mut animations: ResMut<Assets<AnimationClip>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    needs_animator_q: Query<(Entity, &NeedsNifAnimator)>,
    has_parent_q: Query<&ChildOf>,
) {
    for (entity, needs_animator_data) in needs_animator_q.iter() {
        if skeleton_map_res
            .root_skeleton_entity_map
            .get(&needs_animator_data.skeleton_id)
            .is_none()
        {
            return;
        };
        let nif_handle = &needs_animator_data.handle;
        // Check if the asset for this entity is loaded
        let Some(nif) = nif_assets.get(&*nif_handle) else {
            continue;
        };
        //print_animations(&nif.raw_parsed);
        let Ok(nif_animations_map) = extract_animations_from_base_anim(&nif.raw_parsed) else {
            commands.entity(entity).remove::<NeedsNifAnimator>();
            continue;
        };
        /*for (name, _) in &nif_animations_map {
               println!("name: {}", name);
        }*/
        let mut bone_masks: HashMap<String, u32> = HashMap::new();
        let animation_player = AnimationPlayer::default();
        let mut animation_graph = AnimationGraph::new();
        let mut animations_hashmap = HashMap::new();
        let blend_node = animation_graph.add_blend(0.5, animation_graph.root);
        let mut affected_bones_map: HashMap<AnimationNodeIndex, Vec<String>> = HashMap::new();
        for (id, skeleton) in &skeleton_map_res.skeletons {
            if *id == needs_animator_data.skeleton_id {
                let mut mask_group: u32 = 0;
                for bone_data in &skeleton.bones {
                    bone_masks.insert(bone_data.name.to_string(), mask_group);
                    if mask_group >= 63 {
                        panic!("Not designed for nifs with greater than 64 bones!");
                    }
                    mask_group = bone_data.id.0 as u32;
                }
            }
        }
        for (_, nif_animation) in nif_animations_map {
            let mut one_or_two_sequences: Vec<AnimationSequence> = Vec::new();

            if let Some((starting_anim, looping_anim)) = split_animation_for_looping(&nif_animation)
            {
                one_or_two_sequences.push(starting_anim);
                one_or_two_sequences.push(looping_anim);
            } else {
                one_or_two_sequences.push(nif_animation);
            }
            for mut sequence in one_or_two_sequences {
                let seq_name = sequence.name.clone();
                process_nif_animation(
                    &mut sequence,
                    needs_animator_data,
                    &skeleton_map_res,
                    &bone_masks,
                    &has_parent_q,
                    &mut commands,
                    &entity,
                    &mut animation_graph,
                    &mut animations,
                    &blend_node,
                    seq_name,
                    &mut animations_hashmap,
                    &mut affected_bones_map,
                );
            }
        }
        let animation_graph_handle = animation_graphs.add(animation_graph);
        commands
            .entity(entity)
            .insert(AnimationGraphHandle(animation_graph_handle));
        commands.entity(entity).insert(animation_player);
        commands.entity(entity).insert(NifAnimator {
            skeleton_id: needs_animator_data.skeleton_id,
            animations: animations_hashmap,
            affected_bones: affected_bones_map,
            bone_masks,
        });
        commands.entity(entity).remove::<NeedsNifAnimator>();
    }
}
pub fn find_bone_path(has_parent_q: &Query<&ChildOf>, entity: &Entity, mut path: String) -> String {
    path.push_str(&entity.to_string());
    if let Ok(parent) = has_parent_q.get(*entity) {
        find_bone_path(has_parent_q, &parent.parent(), path)
    } else {
        path
    }
}
pub fn extract_animations_from_base_anim(
    nif_data: &ParsedNifData,
) -> Result<HashMap<String, AnimationSequence>, String> {
    let mut animation_sequences: HashMap<String, AnimationSequence> = HashMap::new();
    let mut all_bone_controllers: HashMap<usize, Vec<&NiKeyframeController>> = HashMap::new();

    // 1. Find all NiKeyframeController blocks and group them by their target node index
    for block in &nif_data.blocks {
        if let ParsedBlock::KeyframeController(kfc) = block {
            if let Some(target_index) = kfc.target {
                all_bone_controllers
                    .entry(target_index)
                    .or_default()
                    .push(kfc);
            }
        }
    }

    // 2. Find the root node (e.g., "Bip01") and its NiTextKeyExtraData for sequence definitions
    //    This part is crucial for Morrowind's base_anim.nif
    let mut root_node_index: Option<usize> = None;
    for (i, block) in nif_data.blocks.iter().enumerate() {
        if let ParsedBlock::Node(node) = block {
            // NiNode contains NiAVObject which contains NiObjectNET (name)
            if node.av_base.net_base.name.eq_ignore_ascii_case("Bip01") ||
               node.av_base.net_base.name.eq_ignore_ascii_case("Root Bone") || // Common root names
               node.av_base.net_base.name.eq_ignore_ascii_case("Scene Root")
            {
                // Check if this node has NiTextKeyExtraData directly or indirectly
                if node.extra_data_link.is_some() {
                    // TODO: You need a robust way to traverse the extra data chain.
                    // For now, let's assume the first extra data is the TextKey one if it exists.
                    let mut current_extra_link = node.av_base.net_base.extra_data_link;
                    while let Some(extra_idx) = current_extra_link {
                        if let Some(ParsedBlock::TextKeyExtraData(_)) =
                            nif_data.blocks.get(extra_idx)
                        {
                            root_node_index = Some(i);
                            break;
                        }
                        // Traverse the extra data linked list
                        if let Some(extra_block) = get_block(nif_data, Some(extra_idx), |b| match b
                        {
                            ParsedBlock::TextKeyExtraData(tked) => {
                                Some(&tked.extra_base as &ExtraFields)
                            }
                            ParsedBlock::StringExtraData(sed) => {
                                Some(&sed.extra_base as &ExtraFields)
                            }
                            // TODO: Add other NiExtraData variants if they exist and can form a chain
                            _ => None,
                        }) {
                            current_extra_link = extra_block.next_extra_data_link;
                        } else {
                            break; // Link is invalid or points to a non-extra data block
                        }
                    }
                }
                if root_node_index.is_some() {
                    break;
                }
            }
        }
    }

    if root_node_index.is_none() {
        // Fallback: Look for NiSequenceStreamHelper if Bip01 with TextKeys isn't found
        // This is less common for Morrowind's base_anim.nif character animations
        for (_, block) in nif_data.blocks.iter().enumerate() {
            if let ParsedBlock::SequenceStreamHelper(_) = block {
                // NiSequenceStreamHelper implies animations are defined elsewhere,
                // often linked via its NiObjectNET controller_link.
                // The C++ code has NiSequence and NiControllerSequence which would be
                // the place to look for animation definitions in newer NIFs.
                // For base_anim.nif (v4.0.0.2), this path is less likely for character anims.
                // If it *is* used, the structure is different, involving mTextKeys in NiSequence.
                // TODO: If NiSequenceStreamHelper IS the root for animations in your specific file,
                // you'll need to adapt the logic here based on how NiSequence (not present in your Rust structs)
                // or older structures define sequences.
                // For now, we'll prioritize the TextKeyExtraData route.
                // A simple check for its controller might point to a NiTimeController chain.
                warn!(
                    "Found NiSequenceStreamHelper, but its animation structure is typically different from base_anim.nif bone animations."
                );
            }
        }
        // If still no root_node_index, it might be that animations are not structured around TextKeyExtraData on Bip01.
        // This would be unusual for Morrowind character animations.
        if root_node_index.is_none() {
            return Err("Could not find a suitable root node (e.g., 'Bip01' with TextKeyExtraData or NiSequenceStreamHelper) for animations.".to_string());
        }
    }

    let root_node_block = nif_data
        .blocks
        .get(root_node_index.unwrap())
        .ok_or("Invalid root node index")?;
    let node_as_obj_net = match root_node_block {
        ParsedBlock::Node(n) => &n.av_base.net_base,
        _ => return Err("Root block is not a NiNode".to_string()),
    };

    let mut text_key_data: Option<&NiTextKeyExtraData> = None;
    let mut current_extra_link = node_as_obj_net.extra_data_link;
    while let Some(extra_idx) = current_extra_link {
        if let Some(tked) = get_block(nif_data, Some(extra_idx), as_text_key_extra_data) {
            text_key_data = Some(tked);
            break;
        }
        if let Some(extra_block_base) = get_block(nif_data, Some(extra_idx), |b| match b {
            ParsedBlock::TextKeyExtraData(tked) => {
                Some(&tked.extra_base as &crate::extra_data::ExtraFields)
            }
            ParsedBlock::StringExtraData(sed) => {
                Some(&sed.extra_base as &crate::extra_data::ExtraFields)
            }
            // TODO: Add other NiExtraData variants if they can be in the chain
            _ => None,
        }) {
            current_extra_link = extra_block_base.next_extra_data_link;
        } else {
            break;
        }
    }

    let text_keys = match text_key_data {
        Some(data) => &data.text_keys,
        None => return Err("No NiTextKeyExtraData found for the root animation node.".to_string()),
    };

    // 3. Parse Text Keys to define animation sequences (names, start/stop times)
    //    The format of text keys can vary. Common Morrowind pattern: "animName:start", "animName:stop"
    //    Or sometimes just "loop start", "loop stop" defining one sequence.
    let mut active_data_opt: Option<ActiveSequenceData> = None;
    for text_key in text_keys {
        let tk_value_original = &text_key.value;
        let tk_value_lower = tk_value_original.to_lowercase();
        let tk_time = text_key.time;

        // Extract potential prefix (AnimName) and suffix (command like "start", "stop")
        let mut parts = tk_value_lower.splitn(2, ':');
        let prefix = parts.next().unwrap_or("").trim().to_string();
        let suffix = parts
            .next()
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        // --- Handle "AnimName:start" ---
        if !prefix.is_empty() && suffix == "start" {
            // --- A new animation is starting ---

            // 1. Finalize any PREVIOUS active animation
            if let Some(existing_active_data) = active_data_opt.take() {
                let mut determined_stop_time = tk_time; // Default: new animation's start time implicitly stops the old one.
                let mut used_explicit_stop_for_existing = false;

                // Search within the collected keys of the existing animation for its own stop key
                for (key_time, key_value) in
                    existing_active_data.raw_text_keys_in_sequence.iter().rev()
                {
                    // Iterate in reverse to find last stop key first
                    let lower_key_value = key_value.to_lowercase();
                    let mut parts = lower_key_value.splitn(2, ':');
                    let p = parts.next().unwrap_or("").trim();
                    let s = parts.next().map(|val| val.trim()).unwrap_or_default();

                    if p == existing_active_data.name.to_lowercase() && s == "stop" {
                        // Found its own stop key within its collected raw keys.
                        if *key_time <= tk_time {
                            // Make sure this explicit stop isn't after the new anim starts
                            determined_stop_time = *key_time;
                            used_explicit_stop_for_existing = true;
                        } else {
                            println!(
                                "Warning: Animation '{}' (started {:.3}) had an explicit stop key at {:.3} which is AFTER it was cut short by new animation '{}' starting at {:.3}. Using implicit stop time {:.3}.",
                                existing_active_data.name,
                                existing_active_data.abs_start_time,
                                *key_time,
                                prefix,
                                tk_time,
                                tk_time
                            );
                            // determined_stop_time remains tk_time (new anim start time)
                        }
                        break; // Found the most relevant stop key for the existing animation
                    }
                }

                if !used_explicit_stop_for_existing {
                    println!(
                        "Info: Animation '{}' (started {:.3}) was implicitly stopped at {:.3} by new animation '{}' starting. No prior explicit stop key was found or applicable for it.",
                        existing_active_data.name,
                        existing_active_data.abs_start_time,
                        tk_time,
                        prefix
                    );
                }

                // The actual "Warning: Animation '...' did not have an explicit stop key. Using fallback..."
                // would ideally come from finalize_sequence_logic if it still can't determine a good duration
                // based on `determined_stop_time` and its internal NIF data/curves.
                finalize_sequence_logic(
                    &mut animation_sequences,
                    existing_active_data,
                    determined_stop_time, // Use the determined stop time
                    nif_data,
                    &all_bone_controllers,
                )?;
            }

            // 2. Start the NEW active sequence
            active_data_opt = Some(ActiveSequenceData {
                name: prefix.clone(),
                abs_start_time: tk_time,
                raw_text_keys_in_sequence: vec![(tk_time, tk_value_original.clone())], // Start with its own "start" key
            });
        } else if !prefix.is_empty() && suffix == "stop" {
            // --- An explicit "stop" key is encountered ---
            // We need to see if this stop key belongs to the currently active animation.
            // If it does, we won't finalize immediately. Instead, we just add this stop key
            // to its list. The finalization will occur when the *next* animation starts,
            // or at the very end of all keys. This ensures all its keys are collected.
            // However, the original code's "Orphaned stop key" logic implies immediate taking.
            // To align with fixing the "orphaned" issue as described (where active_data_opt is already None):
            // The key is still to ensure that when an anim *is* finalized implicitly, it searches its collected keys.
            // The "stop" branch here should ideally only finalize if it's a direct match.

            let current_key_is_for_active_anim = if let Some(ad) = &active_data_opt {
                ad.name.to_lowercase() == prefix.to_lowercase()
            } else {
                false
            };

            if current_key_is_for_active_anim {
                // This stop key is for the currently active animation. Add it to its keys.
                // It will be used when this animation is eventually finalized.
                if let Some(ref mut ad) = active_data_opt {
                    ad.raw_text_keys_in_sequence
                        .push((tk_time, tk_value_original.clone()));
                }
            } else {
                // This stop key is NOT for the currently active animation (or no animation is active).
                // This is where "orphaned" or "mismatched" stop keys arise.
                // If an animation *was* active but this stop key is for something else:
                if let Some(mismatched_active_data) = active_data_opt.take() {
                    // The currently active animation is being implicitly stopped by a mismatched stop key.
                    // We should try to find its *own* stop key among its raw_text_keys_in_sequence, if any,
                    // that occurs before or at `tk_time` (time of this mismatched stop key).
                    let mut determined_stop_time = tk_time;
                    let mut used_explicit_stop_for_mismatched = false;
                    // (Similar search logic as in the "start" block for existing_active_data)
                    for (key_time, key_value) in mismatched_active_data
                        .raw_text_keys_in_sequence
                        .iter()
                        .rev()
                    {
                        let lower_key_value = key_value.to_lowercase();
                        let mut parts = lower_key_value.splitn(2, ':');
                        let p = parts.next().unwrap_or("").trim();
                        let s = parts.next().map(|val| val.trim()).unwrap_or_default();
                        if p == mismatched_active_data.name.to_lowercase() && s == "stop" {
                            if *key_time <= tk_time {
                                determined_stop_time = *key_time;
                                used_explicit_stop_for_mismatched = true;
                                println!(
                                    "Info: Active animation '{}' (ending due to mismatched stop) using its own explicit stop at {:.3}.",
                                    mismatched_active_data.name, determined_stop_time
                                );
                            } else {
                                println!(
                                    "Warning: Active animation '{}' (ending due to mismatched stop) had explicit stop at {:.3} but mismatched stop key '{}' is earlier at {:.3}.",
                                    mismatched_active_data.name,
                                    *key_time,
                                    tk_value_original,
                                    tk_time
                                );
                            }
                            break;
                        }
                    }
                    if !used_explicit_stop_for_mismatched {
                        println!(
                            "Info: Active animation '{}' implicitly stopped by mismatched stop key '{}' at {:.3}.",
                            mismatched_active_data.name, tk_value_original, tk_time
                        );
                    }

                    finalize_sequence_logic(
                        &mut animation_sequences,
                        mismatched_active_data, // The one that was active
                        determined_stop_time, // Time of the mismatched stop key, or its own found explicit stop
                        nif_data,
                        &all_bone_controllers,
                    )?;
                    println!(
                        "Orphaned/mismatched stop key: '{}' (for animation '{}') at time {:.3}. Active animation was finalized.",
                        tk_value_original, prefix, tk_time
                    );
                } else {
                    // No active animation was present, so this stop key is truly orphaned.
                    println!(
                        "Orphaned stop key: '{}' at time {:.3} (No active animation).",
                        tk_value_original, tk_time
                    );
                }
            }
        } else {
            // --- Handle other keys (Loop Start, SoundGen, non-start/stop AnimName:Suffix, etc.) ---
            if let Some(ref mut ad) = active_data_opt {
                // Add key if it's for the current animation OR a generic key like SoundGen
                // You might need more specific logic here to decide which keys get added.
                let tk_prefix_lower = prefix.to_lowercase();
                if tk_prefix_lower == ad.name.to_lowercase() || tk_prefix_lower == "soundgen" {
                    ad.raw_text_keys_in_sequence
                        .push((tk_time, tk_value_original.clone()));
                } else if prefix.is_empty() && tk_value_lower.starts_with("soundgen:") {
                    // Handle cases like "SoundGen:Left" where prefix might be empty
                    ad.raw_text_keys_in_sequence
                        .push((tk_time, tk_value_original.clone()));
                }
                // Else: key is for a different, non-active animation and isn't start/stop. Or junk. Ignore or log.
            }
        }
    }
    Ok(animation_sequences)
}
fn finalize_sequence_logic(
    animation_sequences_map: &mut HashMap<String, AnimationSequence>,
    active_data: ActiveSequenceData,
    final_abs_stop_time: f32,
    nif_data: &ParsedNifData,
    all_bone_controllers: &HashMap<usize, Vec<&NiKeyframeController>>,
) -> Result<(), String> {
    let sequence_abs_start_time = active_data.abs_start_time;
    let sequence_name = active_data.name;

    // Ensure stop time is not before start time
    let actual_abs_stop_time = if final_abs_stop_time < sequence_abs_start_time {
        println!(
            "Warning: Correcting stop time for animation '{}'. Original stop {:.3} < start {:.3}. Using start time.",
            sequence_name, final_abs_stop_time, sequence_abs_start_time
        );
        sequence_abs_start_time // or sequence_abs_start_time + a_small_epsilon if zero-duration is problematic
    } else {
        final_abs_stop_time
    };

    let mut events = Vec::new();
    let mut loop_start_abs: Option<f32> = None;
    let mut loop_stop_abs: Option<f32> = None;

    // Sort raw keys by time before processing for loop markers and events
    let mut sorted_raw_keys = active_data.raw_text_keys_in_sequence;
    sorted_raw_keys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    for (key_abs_time, key_original_value) in &sorted_raw_keys {
        // Only consider keys within the actual span of this sequence
        if *key_abs_time < sequence_abs_start_time || *key_abs_time > actual_abs_stop_time {
            continue;
        }
        for key_original_value in key_original_value.lines() {
            let key_value_lower = key_original_value.to_lowercase();
            let mut parts = key_value_lower.splitn(2, ':');
            let prefix = parts.next().unwrap_or("").trim();
            let suffix = parts.next().map(|s| s.trim()).unwrap_or("");

            let is_relevant_to_current_anim = prefix == sequence_name || prefix.is_empty();
            if is_relevant_to_current_anim && suffix == "loop start" {
                loop_start_abs = Some(*key_abs_time);
            } else if is_relevant_to_current_anim && suffix == "loop stop" {
                loop_stop_abs = Some(*key_abs_time);
            } else if !(is_relevant_to_current_anim && (suffix == "start" || suffix == "stop")) {
                // If it's not a start/stop marker specifically for *this* animation sequence's
                // main boundaries (which are already determined), then it's an event.
                events.push(TextKeyEvent {
                    time: *key_abs_time - sequence_abs_start_time, // Make time relative
                    value: key_original_value.to_string(),
                });
            }
        }
    }
    // Ensure events are sorted by relative time (though they should be if raw keys were sorted by abs time)
    events.sort_by(|a, b| {
        a.time
            .partial_cmp(&b.time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut current_sequence = AnimationSequence {
        name: sequence_name.clone(),
        abs_start_time: sequence_abs_start_time,
        abs_stop_time: actual_abs_stop_time,
        bone_curves: Vec::new(), // Will be populated below
        events,
        loop_start_time: loop_start_abs.map(|t| t - sequence_abs_start_time),
        loop_stop_time: loop_stop_abs.map(|t| t - sequence_abs_start_time),
        initial_position: Vec3::new(0.0, 0.0, 0.0),
        is_startup_to_loop: false,
    };

    // --- Validate and adjust loop times (make them relative and sane) ---
    let sequence_duration_relative =
        current_sequence.abs_stop_time - current_sequence.abs_start_time;

    if let Some(lst) = &mut current_sequence.loop_start_time {
        if *lst < 0.0 || *lst > sequence_duration_relative {
            println!(
                "Warning: Relative loop_start_time {:.3} out of bounds for '{}' (duration {:.3}). Removing loop.",
                *lst, current_sequence.name, sequence_duration_relative
            );
            current_sequence.loop_start_time = None;
            current_sequence.loop_stop_time = None;
        }
    }
    if let Some(let_stop) = &mut current_sequence.loop_stop_time {
        let start_loop_val = current_sequence.loop_start_time.unwrap_or(0.0); // Default to 0 if only stop exists
        if *let_stop < start_loop_val || *let_stop > sequence_duration_relative {
            println!(
                "Warning: Relative loop_stop_time {:.3} invalid for '{}' (start {:.3}, duration {:.3}). Removing loop_stop.",
                *let_stop, current_sequence.name, start_loop_val, sequence_duration_relative
            );
            current_sequence.loop_stop_time = None;
            // If loop_start was valid but now stop is gone, NIF usually loops to end of sequence
            if current_sequence.loop_start_time.is_some() {
                current_sequence.loop_stop_time = Some(sequence_duration_relative);
                println!(
                    "Adjusted loop_stop_time for '{}' to sequence end: {:.3}",
                    current_sequence.name, sequence_duration_relative
                );
            }
        }
    }
    // If loop_start exists but loop_stop doesn't, standard behavior is to loop to the end of the sequence.
    if current_sequence.loop_start_time.is_some() && current_sequence.loop_stop_time.is_none() {
        current_sequence.loop_stop_time = Some(sequence_duration_relative);
    }

    // --- Populate bone_curves (using the sequence's absolute start/stop times) ---
    for (target_node_idx, controllers) in all_bone_controllers {
        let target_node_block = nif_data.blocks.get(*target_node_idx).ok_or_else(|| {
            format!(
                "Invalid target node index {} for controller",
                target_node_idx
            )
        })?;

        let bone_name = match target_node_block {
            ParsedBlock::Node(node) => node.av_base.net_base.name.clone(),
            _ => format!("UnnamedBone_{}", target_node_idx), // Fallback
        };

        let mut bone_curve = BoneAnimationCurve::default();
        bone_curve.target_bone_name = bone_name;

        for kfc in controllers {
            if let Some(kfd_block_idx) = kfc.keyframe_data {
                if let Some(keyframe_data) =
                    get_block(nif_data, Some(kfd_block_idx), as_keyframe_data)
                {
                    for key_quat in &keyframe_data.quaternion_keys {
                        if key_quat.time >= current_sequence.abs_start_time
                            && key_quat.time <= current_sequence.abs_stop_time
                        {
                            bone_curve.rotations.push((
                                key_quat.time - current_sequence.abs_start_time,
                                to_bevy_quat(key_quat.value),
                            ));
                        }
                    }
                    if let Some(key_vec3) = keyframe_data.translations.first() {
                        if key_vec3.time >= current_sequence.abs_start_time
                            && key_vec3.time <= current_sequence.abs_stop_time
                        {
                            current_sequence.initial_position = to_bevy_vec3(key_vec3.value);
                        }
                    }
                    if bone_curve.target_bone_name == "Bip01" {
                        //prevent Bip01 from moving in x and z, so the running animation doesn't
                        //zoom ahead of the player
                        let (initial_x, initial_y) =
                            if let Some(key_vec3) = keyframe_data.translations.first() {
                                let initial_translation = to_bevy_vec3(key_vec3.value);
                                (initial_translation.x, initial_translation.y)
                            } else {
                                (0.0, 0.0)
                            };
                        for key_vec3 in &keyframe_data.translations {
                            if key_vec3.time >= current_sequence.abs_start_time
                                && key_vec3.time <= current_sequence.abs_stop_time
                            {
                                let translation = to_bevy_vec3(key_vec3.value);
                                let fixed_translation =
                                    Vec3::new(initial_x, initial_y, translation.z);
                                bone_curve.translations.push((
                                    key_vec3.time - current_sequence.abs_start_time,
                                    fixed_translation,
                                ));
                            }
                        }
                    } else {
                        for key_vec3 in &keyframe_data.translations {
                            if key_vec3.time >= current_sequence.abs_start_time
                                && key_vec3.time <= current_sequence.abs_stop_time
                            {
                                bone_curve.translations.push((
                                    key_vec3.time - current_sequence.abs_start_time,
                                    to_bevy_vec3(key_vec3.value),
                                ));
                            }
                        }
                    }
                    for key_float in &keyframe_data.scales {
                        if key_float.time >= current_sequence.abs_start_time
                            && key_float.time <= current_sequence.abs_stop_time
                        {
                            bone_curve.scales.push((
                                key_float.time - current_sequence.abs_start_time,
                                key_float.value,
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
                .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            bone_curve
                .translations
                .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            bone_curve
                .scales
                .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            current_sequence.bone_curves.push(bone_curve);
        }
    }
    // --- End Populate bone_curves ---

    if !current_sequence.bone_curves.is_empty() || !current_sequence.events.is_empty() {
        animation_sequences_map.insert(current_sequence.name.clone(), current_sequence);
    } else {
        println!(
            "SKIPPING empty sequence (no curves/events): Name='{}' (AbsStart={:.3}, AbsStop={:.3})",
            sequence_name, sequence_abs_start_time, actual_abs_stop_time
        );
    }
    Ok(())
}

#[allow(dead_code)]
pub fn print_animations(parsed_nif_data: &ParsedNifData) {
    match extract_animations_from_base_anim(parsed_nif_data) {
        Ok(animations) => {
            println!(
                "Successfully extracted {} animation sequences:",
                animations.len()
            );
            for (name, anim_seq) in animations {
                println!(
                    "  Sequence: {}, Start: {:.2}, Stop: {:.2}, Bones Animated: {}",
                    name,
                    anim_seq.abs_start_time,
                    anim_seq.abs_stop_time,
                    anim_seq.bone_curves.len()
                );
                for bone_curve in anim_seq.bone_curves.iter() {
                    // Print details for first bone only
                    println!("    Bone: {}", bone_curve.target_bone_name);
                    if !bone_curve.translations.is_empty() {}
                    if !bone_curve.rotations.is_empty() {
                        println!("      Rotations: {} keys", bone_curve.rotations.len());
                    }
                    if !bone_curve.scales.is_empty() {
                        println!("      Scales: {} keys", bone_curve.scales.len());
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error extracting animations: {}", e);
        }
    }
}
/// Resolves a RecordLink to a specific ParsedBlock and attempts to cast it.
fn get_block<'a, T>(
    nif_data: &'a ParsedNifData,
    link: RecordLink,
    caster: fn(&'a ParsedBlock) -> Option<&'a T>,
) -> Option<&'a T> {
    link.and_then(|index| nif_data.blocks.get(index).and_then(caster))
}
// Your Vector3 might already be compatible or a type alias for Bevy's Vec3
fn to_bevy_vec3(v: crate::base::Vector3) -> bevy::math::Vec3 {
    bevy::math::Vec3::new(v.0[0], v.0[1], v.0[2])
}

// Helper to convert your Quaternion to Bevy's Quat
// Your Quaternion might already be compatible or a type alias for Bevy's Quat
fn to_bevy_quat(q: crate::animation::Quaternion) -> bevy::math::Quat {
    // Assuming your Quaternion stores as [x, y, z, w] which is Bevy's order
    bevy::math::Quat::from_xyzw(q.x, q.y, q.z, q.w) // Adjust if your storage order is different
}

// Specific caster functions

fn as_keyframe_data(block: &ParsedBlock) -> Option<&NiKeyframeData> {
    if let ParsedBlock::KeyframeData(kfd) = block {
        Some(kfd)
    } else {
        None
    }
}

fn as_text_key_extra_data(block: &ParsedBlock) -> Option<&NiTextKeyExtraData> {
    if let ParsedBlock::TextKeyExtraData(tked) = block {
        Some(tked)
    } else {
        None
    }
}
//function to determine if we should use a constant curve or an auto curve
//(based on having only 1 or more than 1 keyframe)
fn make_auto_or_constant_curve<T: Copy>(
    data: &Vec<(f32, T)>,
    interval: Result<Interval, InvalidIntervalError>,
) -> (Option<ConstantCurve<T>>, Option<UnevenSampleAutoCurve<T>>) {
    let Ok(interval) = interval else {
        return (None, None);
    };
    match data.len() {
        0 => (None, None),
        1 => {
            if let Some(first) = data.first() {
                (Some(ConstantCurve::new(interval, first.1)), None)
            } else {
                (None, None)
            }
        }
        _ => {
            if let Ok(curve) = UnevenSampleAutoCurve::new(data.iter().copied()) {
                (None, Some(curve))
            } else {
                (None, None)
            }
        }
    }
}
fn filter_intro_track<T: Clone>(
    original_track: &[(f32, T)],
    intro_end_time_exclusive: f32, // Marks the point strictly before which keyframes are kept
) -> Vec<(f32, T)> {
    original_track
        .iter()
        .filter(|(time, _)| *time < intro_end_time_exclusive) // Keyframes strictly before the loop starts
        .cloned()
        .collect()
}
fn filter_and_retime_loop_track<T: Clone>(
    original_track: &[(f32, T)],
    loop_segment_start_time: f32, // Start of the loop segment in original sequence time
    loop_segment_end_time: f32,   // End of the loop segment in original sequence time
) -> Vec<(f32, T)> {
    original_track
        .iter()
        .filter_map(|(time, value)| {
            // Include keyframes within the loop segment (inclusive, with a small tolerance for f32 comparison)
            if *time >= loop_segment_start_time - 1e-4 && *time <= loop_segment_end_time + 1e-4 {
                Some((*time - loop_segment_start_time, value.clone())) // Adjust time to be 0-based for the loop
            } else {
                None
            }
        })
        .collect()
}
pub fn split_animation_for_looping(
    original_sequence: &AnimationSequence,
) -> Option<(AnimationSequence, AnimationSequence)> {
    let original_relative_duration =
        original_sequence.abs_stop_time - original_sequence.abs_start_time;
    if original_relative_duration < 0.0 {
        // Basic sanity check
        return None;
    }
    let (mut loop_start_rel, loop_stop_rel) = match (
        original_sequence.loop_start_time,
        original_sequence.loop_stop_time,
    ) {
        (Some(start), Some(stop))
            if start >= 0.0 && start < stop && stop <= original_relative_duration + 1e-4 =>
        {
            (start, stop)
        }
        _ => {
            /*eprintln!(
                "Cannot split animation '{}': Invalid or missing loop_start_time/loop_stop_time, or loop segment is invalid/out of bounds.",
                original_sequence.name
            );*/
            return None;
        }
    };

    loop_start_rel += 1.0 / 24.0;
    // --- Create Intro Sequence ---
    let intro_name = original_sequence.name.clone();
    let intro_abs_start_time = original_sequence.abs_start_time;
    let intro_abs_stop_time = original_sequence.abs_start_time + loop_start_rel;
    // intro_duration_rel is loop_start_rel

    let mut intro_bone_curves: Vec<BoneAnimationCurve> = Vec::new();
    for original_curve in &original_sequence.bone_curves {
        let intro_rotations = filter_intro_track(&original_curve.rotations, loop_start_rel);
        let intro_translations = filter_intro_track(&original_curve.translations, loop_start_rel);
        let intro_scales = filter_intro_track(&original_curve.scales, loop_start_rel);

        // O-ly add the curve if it had content or if the intro itself is zero-duration (to keep structure)
        if !intro_rotations.is_empty()
            || !intro_translations.is_empty()
            || !intro_scales.is_empty()
            || loop_start_rel == 0.0
        {
            intro_bone_curves.push(BoneAnimationCurve {
                target_bone_name: original_curve.target_bone_name.clone(),
                rotations: intro_rotations,
                translations: intro_translations,
                scales: intro_scales,
            });
        }
    }

    let intro_events: Vec<TextKeyEvent> = original_sequence
        .events
        .iter()
        .filter(|event| event.time < loop_start_rel)
        .cloned()
        .collect();

    let intro_sequence = AnimationSequence {
        name: intro_name,
        abs_start_time: intro_abs_start_time,
        abs_stop_time: intro_abs_stop_time,
        bone_curves: intro_bone_curves,
        events: intro_events,
        loop_start_time: None,
        loop_stop_time: None,
        initial_position: original_sequence.initial_position,
        is_startup_to_loop: true,
    };

    loop_start_rel -= 1.0 / 24.0;
    // --- Create Loop Sequence ---
    let loop_seq_name = format!("{}_loop", original_sequence.name);
    let loop_seq_abs_start_time = original_sequence.abs_start_time + loop_start_rel;
    let loop_seq_abs_stop_time = original_sequence.abs_start_time + loop_stop_rel;
    let loop_seq_duration_rel = loop_stop_rel - loop_start_rel;

    let mut loop_seq_bone_curves: Vec<BoneAnimationCurve> = Vec::new();
    for original_curve in &original_sequence.bone_curves {
        let loop_rotations =
            filter_and_retime_loop_track(&original_curve.rotations, loop_start_rel, loop_stop_rel);
        let loop_translations = filter_and_retime_loop_track(
            &original_curve.translations,
            loop_start_rel,
            loop_stop_rel,
        );
        let loop_scales =
            filter_and_retime_loop_track(&original_curve.scales, loop_start_rel, loop_stop_rel);

        if !loop_rotations.is_empty()
            || !loop_translations.is_empty()
            || !loop_scales.is_empty()
            || loop_seq_duration_rel == 0.0
        {
            loop_seq_bone_curves.push(BoneAnimationCurve {
                target_bone_name: original_curve.target_bone_name.clone(),
                rotations: loop_rotations,
                translations: loop_translations,
                scales: loop_scales,
            });
        }
    }

    let loop_seq_events: Vec<TextKeyEvent> = original_sequence
        .events
        .iter()
        .filter_map(|event| {
            if event.time >= loop_start_rel - 1e-4 && event.time <= loop_stop_rel + 1e-4 {
                Some(TextKeyEvent {
                    time: event.time - loop_start_rel, // Adjust time
                    value: event.value.clone(),
                })
            } else {
                None
            }
        })
        .collect();

    let loop_sequence = AnimationSequence {
        name: loop_seq_name,
        abs_start_time: loop_seq_abs_start_time,
        abs_stop_time: loop_seq_abs_stop_time,
        bone_curves: loop_seq_bone_curves,
        events: loop_seq_events,
        loop_start_time: Some(0.0),
        loop_stop_time: Some(loop_seq_duration_rel),
        initial_position: original_sequence.initial_position,
        is_startup_to_loop: false,
    };

    Some((intro_sequence, loop_sequence))
}
fn process_nif_animation(
    nif_animation: &mut AnimationSequence,
    needs_animator_data: &NeedsNifAnimator,
    skeleton_map_res: &Res<SkeletonMap>,
    bone_masks: &HashMap<String, u32>,
    has_parent_q: &Query<&ChildOf>,
    commands: &mut Commands,
    entity: &Entity,
    animation_graph: &mut AnimationGraph,
    animations: &mut ResMut<Assets<AnimationClip>>,
    blend_node: &AnimationNodeIndex,
    name: String,
    animations_hashmap: &mut HashMap<String, AnimationNodeIndex>,
    affected_bones_map: &mut HashMap<AnimationNodeIndex, Vec<String>>,
) {
    let mut affected_bones = Vec::new();
    let mut mask_group = 0;
    let mut animation_clip = AnimationClip::default();
    let mut bone_entity: Option<Entity>;
    for bone_curve in &mut nif_animation.bone_curves.iter_mut() {
        /*   if bone_curve.target_bone_name == "Bip01" {
            for (_duration, bone_translation) in &mut bone_curve.translations {
                *bone_translation = Vec3::new(
                    nif_animation.initial_position.x,
                    nif_animation.initial_position.y,
                    nif_animation.initial_position.z,
                );
            }
        }*/
        let translation_curves = make_auto_or_constant_curve(
            &bone_curve.translations,
            Interval::new(nif_animation.abs_start_time, nif_animation.abs_stop_time),
        );
        bone_entity = None;
        let rotation_curves = make_auto_or_constant_curve(
            &bone_curve.rotations,
            Interval::new(nif_animation.abs_start_time, nif_animation.abs_stop_time),
        );
        if let Some(skeleton) = skeleton_map_res
            .skeletons
            .get(&needs_animator_data.skeleton_id)
        {
            if let Some(bone_data) = skeleton.get_bone_by_name(&bone_curve.target_bone_name) {
                affected_bones.push(bone_curve.target_bone_name.clone());
                mask_group = *bone_masks.get(&bone_curve.target_bone_name).unwrap();
                bone_entity = Some(bone_data.entity);
            }
        }
        let Some(bone_entity) = bone_entity else {
            continue;
        };
        let path = String::from("");
        let bone_path = find_bone_path(&has_parent_q, &bone_entity, path);
        let target_id = AnimationTargetId::from_name(&Name::new(bone_path));
        if let (Some(auto_curve), _) = translation_curves {
            animation_clip.add_curve_to_target(
                target_id,
                AnimatableCurve::new(animated_field!(Transform::translation), auto_curve),
            );
        } else if let (_, Some(constant_curve)) = translation_curves {
            animation_clip.add_curve_to_target(
                target_id,
                AnimatableCurve::new(animated_field!(Transform::translation), constant_curve),
            );
        }
        if let (Some(auto_curve), _) = rotation_curves {
            animation_clip.add_curve_to_target(
                target_id,
                AnimatableCurve::new(animated_field!(Transform::rotation), auto_curve),
            );
        } else if let (_, Some(constant_curve)) = rotation_curves {
            animation_clip.add_curve_to_target(
                target_id,
                AnimatableCurve::new(animated_field!(Transform::rotation), constant_curve),
            );
        }

        commands.entity(bone_entity).insert(AnimationTarget {
            id: target_id,
            player: *entity,
        });
        animation_graph.add_target_to_mask_group(target_id, mask_group as u32);
    }
    for event in &nif_animation.events {
        if event.value.contains("SoundGen") {
            animation_clip.add_event(
                event.time,
                NifEvent {
                    skeleton_id: needs_animator_data.skeleton_id,
                    event_type: NifEventType::PlaySound {
                        sound_name: event.value.clone(),
                    },
                },
            );
        }
    }
    if let Some(loop_start) = nif_animation.loop_start_time {
        if let Some(loop_end) = nif_animation.loop_stop_time {
            if loop_start == loop_end {
                //if the loop start and end time are the same, this loops a frozen animation
                animation_clip.add_event(
                    loop_start,
                    NifEvent {
                        skeleton_id: needs_animator_data.skeleton_id,
                        event_type: NifEventType::FreezeAnimation {
                            animation_name: nif_animation.name.clone(),
                        },
                    },
                );
                //return to idle once the animation is finished
                animation_clip.add_event(
                    nif_animation.abs_stop_time - nif_animation.abs_start_time,
                    NifEvent {
                        skeleton_id: needs_animator_data.skeleton_id,
                        event_type: NifEventType::ResumeIdle {
                            animation_name: nif_animation.name.clone(),
                        },
                    },
                );
            }
        }
    }
    if nif_animation.is_startup_to_loop {
        animation_clip.add_event(
            nif_animation.abs_stop_time - (nif_animation.abs_start_time + 1.0 / 24.0),
            NifEvent {
                skeleton_id: needs_animator_data.skeleton_id,
                event_type: NifEventType::LoopAnimation {
                    animation_name: nif_animation.name.clone(),
                },
            },
        );
    }
    let handle = animations.add(animation_clip);
    let animation_node = animation_graph.add_clip(handle, 1.0, *blend_node);
    animations_hashmap.insert(name, animation_node);
    affected_bones_map.insert(animation_node, affected_bones);
}
