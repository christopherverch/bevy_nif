use bevy::prelude::*;
use bevy_animation::{AnimationTarget, AnimationTargetId, animated_field};
use std::collections::{HashMap, HashSet};

use crate::{
    NiKeyframeController,
    nif::{
        loader::Nif,
        parser::helpers::find_node_by_name_recursive,
        spawner::NeedsNifAnimator,
        types::{ParsedBlock, TextKey},
    },
    nif_animation::{
        AnimationDefinition, NifAnimator, NifAnimatorAdded, NifEvent, NifEventType,
        REGION_ROOT_LOWER_BODY,
        bevy_types::Priority,
        parser_helpers::{
            determine_bone_primary_region_index, filter_and_retime_keyframes,
            is_inherently_looping, make_bevy_curve, sample_vec3_curve,
        },
    },
};

use super::SkeletonMap;

/// A flattened, simplified representation of a single command from a text key.
#[derive(Debug, Clone)]
struct ParsedKeyEvent {
    time: f32,
    original_line: String,
    name: String,
    command: String,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ProcessedAnimation {
    pub name: String,
    pub start_time: f32,
    pub end_time: f32,
    pub events: Vec<String>,
}

/// A temporary struct to hold all data about a block before the final split decision.
#[derive(Debug, Clone)]
struct AnimationBlockData {
    name: String,
    start_time: f32,
    end_time: f32,
    loop_start_time: Option<f32>,
    loop_end_time: Option<f32>,
}
#[derive(Debug, Clone)]
struct RawBoneAnimation {
    target_id: AnimationTargetId,
    bone_entity: Entity,
    bone_name: String,
    is_bip01: bool,
    /// All rotation keys for this bone, sorted by time.
    all_rotation_keys: Vec<(f32, Quat)>,
    /// All translation keys for this bone, sorted by time.
    all_translation_keys: Vec<(f32, Vec3)>,
    // NOTE: Add `all_scale_keys` here if we need to support scaling.
}

pub fn setup_animations(
    needs_animator_q: Query<(Entity, &NeedsNifAnimator)>,
    nif_assets: Res<Assets<Nif>>,
    skeleton_map_res: Res<SkeletonMap>,
    mut bevy_animation_clips: ResMut<Assets<AnimationClip>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    mut commands: Commands,
) {
    if needs_animator_q.is_empty() {
        return;
    }

    for (entity, needs_animator_data) in needs_animator_q.iter() {
        let nif_handle = &needs_animator_data.handle;
        let Some(nif_asset) = nif_assets.get(nif_handle) else {
            continue;
        };
        let Some(skeleton) = skeleton_map_res
            .skeletons
            .get(&needs_animator_data.skeleton_id)
        else {
            continue;
        };

        // --- STEP 1: Extract Bone Controllers ---
        let mut all_bone_controllers: HashMap<usize, Vec<NiKeyframeController>> = HashMap::new();
        for block in &nif_asset.raw_parsed.blocks {
            if let ParsedBlock::KeyframeController(kfc) = block {
                if let Some(target_index) = kfc.target {
                    all_bone_controllers
                        .entry(target_index)
                        .or_default()
                        .push(kfc.clone());
                }
            }
        }

        // --- Text Key Extraction ---
        let global_nif_text_keys: &[TextKey] = 'data_extraction: {
            // 1. Find the animation root ("Bip01") by searching from the file's root nodes.
            let bip01_node = nif_asset
                .raw_parsed
                .root_links
                .iter()
                .find_map(|root_link| {
                    root_link.and_then(|root_index| {
                        // First try to find "Bip01"
                        find_node_by_name_recursive(
                            &nif_asset.raw_parsed.blocks,
                            root_index,
                            "Bip01",
                        )
                        // If that fails, fall back to trying "Root Bone"
                        .or_else(|| {
                            find_node_by_name_recursive(
                                &nif_asset.raw_parsed.blocks,
                                root_index,
                                "Root Bone",
                            )
                        })
                    })
                });

            // 2. If we found the "Bip01" node, traverse its extra data chain.
            if let Some(root_node) = bip01_node {
                let mut current_link = root_node.av_base.net_base.extra_data_link;
                while let Some(block_index) = current_link {
                    let Some(block) = nif_asset.raw_parsed.blocks.get(block_index) else {
                        break; // Chain is broken
                    };

                    // If this block is the TextKey data, we're done.
                    if let ParsedBlock::TextKeyExtraData(tked) = block {
                        // Use `break 'label` to exit the labeled block and return the value.
                        break 'data_extraction &tked.text_keys;
                    }

                    // Otherwise, follow the link to the next item in the chain.
                    current_link = match block {
                        ParsedBlock::TextKeyExtraData(tked) => tked.extra_base.next_extra_data_link,
                        ParsedBlock::StringExtraData(sed) => sed.extra_base.next_extra_data_link,
                        _ => break, // Not an extra data block, chain ends.
                    };
                }
            }

            // If we didn't find the Bip01 node or the TextKeyExtraData block, this is the final value.
            &[]
        };
        if global_nif_text_keys.is_empty() {
            commands.entity(entity).remove::<NeedsNifAnimator>();
            continue;
        }

        // --- Call the Parser ---
        let processed_animations = parse_and_split_animation_blocks(global_nif_text_keys);
        info!(
            "--- Extracting All Bone Keyframes for {:?} (Entity {:?}) ---",
            nif_handle.path(),
            entity
        );
        let loop_base_names: HashSet<String> = processed_animations
            .iter()
            .filter_map(|clip| clip.name.strip_suffix("_loop").map(String::from))
            .collect();

        let mut raw_bone_data: Vec<RawBoneAnimation> = Vec::new();
        let mut tagged_bones = std::collections::HashSet::new();

        for (bone_block_index, controllers) in &all_bone_controllers {
            let Some(ParsedBlock::Node(bone_node)) =
                nif_asset.raw_parsed.blocks.get(*bone_block_index)
            else {
                continue;
            };
            let bone_name = &bone_node.av_base.net_base.name;
            let Some(bone_data) = skeleton.get_bone_by_name(bone_name) else {
                continue;
            };

            let mut bone_anim = RawBoneAnimation {
                target_id: AnimationTargetId::from_name(&Name::new(bone_name.clone())),
                bone_entity: bone_data.entity,
                bone_name: bone_name.clone(),
                is_bip01: bone_name.eq_ignore_ascii_case(REGION_ROOT_LOWER_BODY),
                all_rotation_keys: Vec::new(),
                all_translation_keys: Vec::new(),
            };

            for controller in controllers {
                let Some(keyframe_data_index) = controller.keyframe_data else {
                    continue;
                };
                let Some(ParsedBlock::KeyframeData(keyframe_data)) =
                    nif_asset.raw_parsed.blocks.get(keyframe_data_index)
                else {
                    continue;
                };

                // Just extend the vectors. No processing yet.
                bone_anim.all_rotation_keys.extend(
                    keyframe_data
                        .quaternion_keys
                        .iter()
                        .map(|k| (k.time, k.value)),
                );
                bone_anim
                    .all_translation_keys
                    .extend(keyframe_data.translations.iter().map(|k| (k.time, k.value)));
            }

            // If we found any keys for this bone, sort them by time and store them.
            if !bone_anim.all_rotation_keys.is_empty() || !bone_anim.all_translation_keys.is_empty()
            {
                // Sorting here is crucial for the filtering logic later.
                bone_anim
                    .all_rotation_keys
                    .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                bone_anim
                    .all_translation_keys
                    .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

                // MOVED: Tag the bone with AnimationTarget *once* during extraction.
                if tagged_bones.insert(bone_anim.bone_entity) {
                    commands
                        .entity(bone_anim.bone_entity)
                        .insert(AnimationTarget {
                            id: bone_anim.target_id,
                            player: entity,
                        });
                }

                raw_bone_data.push(bone_anim);
            }
        } // Pre-process and sort the Bip01 translation keys once for fast lookups.
        let bip01_translation_keys: Vec<(f32, Vec3)> = 'find_keys: {
            // Find the Bip01/root bone controller index first.
            let Some((_, bip01_controllers)) = all_bone_controllers.iter().find(|(idx, _)| {
                if let Some(ParsedBlock::Node(node)) = nif_asset.raw_parsed.blocks.get(**idx) {
                    node.av_base
                        .net_base
                        .name
                        .eq_ignore_ascii_case(REGION_ROOT_LOWER_BODY)
                } else {
                    false
                }
            }) else {
                // If there's no Bip01 controller, break with an empty Vec.
                break 'find_keys Vec::new();
            };

            let mut keys = Vec::new();
            for controller in bip01_controllers {
                if let Some(kfd_idx) = controller.keyframe_data {
                    if let Some(ParsedBlock::KeyframeData(kfd)) =
                        nif_asset.raw_parsed.blocks.get(kfd_idx)
                    {
                        for key in &kfd.translations {
                            keys.push((key.time, key.value));
                        }
                    }
                }
            }
            // Sort the keys by time. This is crucial for the binary search.
            keys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            keys
        };
        let animation_player = AnimationPlayer::default();
        let mut animation_graph = AnimationGraph::new();
        let mut animation_definitions_map: HashMap<String, AnimationDefinition> = HashMap::new();
        let root_blend_node = animation_graph.add_blend(0.5, animation_graph.root);

        // MOVED & CHANGED: Add targets to graph mask based on pre-filtered `raw_bone_data`
        for bone in &raw_bone_data {
            let region_idx = determine_bone_primary_region_index(&bone.bone_name, skeleton);
            animation_graph.add_target_to_mask_group(bone.target_id, region_idx as u32);
        }

        // --- This is the new, more efficient loop structure ---
        for processed_clip in &processed_animations {
            let mut root_translation_curve: Option<AnimatableKeyframeCurve<Vec3>> = None;
            let mut bevy_clip = AnimationClip::default();

            // Loop over our pre-collected bone data, NOT the NIF controllers.
            for bone in &raw_bone_data {
                // --- Handle Rotations from pre-collected data ---
                if !bone.all_rotation_keys.is_empty() {
                    let rot_keys = filter_and_retime_keyframes(
                        bone.all_rotation_keys.iter().cloned(), // .cloned() is cheap for tuples of primatives
                        processed_clip.start_time,
                        processed_clip.end_time,
                    );
                    if let Some(curve) = make_bevy_curve(&rot_keys) {
                        bevy_clip.add_curve_to_target(
                            bone.target_id,
                            AnimatableCurve::new(animated_field!(Transform::rotation), curve),
                        );
                    }
                }

                // --- Handle Translations from pre-collected data ---
                if !bone.all_translation_keys.is_empty() {
                    // This part is unchanged: get the correctly timed keys for the current clip segment.
                    let trans_keys = filter_and_retime_keyframes(
                        bone.all_translation_keys.iter().cloned(),
                        processed_clip.start_time,
                        processed_clip.end_time,
                    );

                    if let Some(curve) = make_bevy_curve(&trans_keys) {
                        // THIS IS THE NEW LOGIC:
                        if bone.is_bip01 {
                            // If it's the root bone, store its translation curve in our temporary variable.
                            root_translation_curve = Some(curve);
                        } else {
                            // For any other bone, add its translation to the main animation clip as before.
                            bevy_clip.add_curve_to_target(
                                bone.target_id,
                                // Using your original, correct syntax
                                AnimatableCurve::new(
                                    animated_field!(Transform::translation),
                                    curve,
                                ),
                            );
                        }
                    }
                }
            } // end for bone in raw_bone_data
            // Process and add the events that were parsed for this clip.
            for event_string in &processed_clip.events {
                // The event string is formatted as: "'OriginalLine' @ AbsoluteTime"
                if let Some((line_part, time_part)) = event_string.rsplit_once(" @ ") {
                    if let Ok(absolute_time) = time_part.parse::<f32>() {
                        // The event time must be relative to the clip's start.
                        let relative_time = absolute_time - processed_clip.start_time;
                        let original_line = line_part.trim_matches('\'');

                        // Make sure the event falls within this clip's duration.
                        if relative_time >= 0.0 {
                            // Determine the event type from the original line.
                            if original_line.to_lowercase().contains("soundgen") {
                                bevy_clip.add_event(
                                    relative_time,
                                    NifEvent {
                                        event_type: NifEventType::SoundGen {
                                            sound_name: original_line.to_string(),
                                        },
                                        entity,
                                    },
                                );
                            }
                            // Add else if blocks here to parse other kinds of events,
                            // e.g., if original_line.contains("footstep") { ... }
                        }
                    }
                }
            }
            let clip_duration = bevy_clip.duration();
            let bevy_clip_handle = bevy_animation_clips.add(bevy_clip);
            let mut base_velocity = Vec3::ZERO;

            // Use the helper function you created to check if this clip should define a velocity.
            let is_standalone = is_inherently_looping(&processed_clip.name)
                && !loop_base_names.contains(&processed_clip.name);
            if processed_clip.name.ends_with("_loop") || is_standalone {
                if !bip01_translation_keys.is_empty() {
                    let start_pos_opt =
                        sample_vec3_curve(&bip01_translation_keys, processed_clip.start_time);
                    let end_pos_opt =
                        sample_vec3_curve(&bip01_translation_keys, processed_clip.end_time);
                    let duration = processed_clip.end_time - processed_clip.start_time;

                    if let (Some(start_pos), Some(end_pos)) = (start_pos_opt, end_pos_opt) {
                        if duration > 1e-4 {
                            let displacement = end_pos - start_pos;
                            base_velocity = Vec3::new(
                                displacement.x / duration,
                                displacement.y / duration,
                                0.0,
                            );
                        }
                    }
                }
            }
            let graph_node_index =
                animation_graph.add_clip(bevy_clip_handle.clone(), 1.0, root_blend_node);

            animation_definitions_map.insert(
                processed_clip.name.clone(),
                AnimationDefinition {
                    node_index: graph_node_index,
                    clip_handle: bevy_clip_handle,
                    next_clip_name: None,
                    duration: clip_duration,
                    base_velocity,
                    root_translation_curve,
                },
            );
        }
        // --- Second Pass: Propagate loop velocities to intros and outros ---
        let mut loop_velocities: HashMap<String, Vec3> = HashMap::new();

        // First, collect all the calculated loop velocities, keyed by their base name.
        for (name, def) in &animation_definitions_map {
            if let Some(base_name) = name.strip_suffix("_loop") {
                loop_velocities.insert(base_name.to_string(), def.base_velocity);
            }
        }

        // Now, apply the stored loop velocities to any intros and outros.
        for (name, def) in animation_definitions_map.iter_mut() {
            // Only apply to animations that don't already have a valid velocity.
            // This correctly targets intros and outros, while leaving standalones (like jump) alone.
            if def.base_velocity.length_squared() < 1e-6 {
                // Determine the base name of the animation, regardless of suffix.
                let base_name = name
                    .strip_suffix("_loop")
                    .or_else(|| name.strip_suffix("_outro"))
                    .unwrap_or(name); // If no suffix, the name is the base name.

                // If we found a corresponding loop velocity, apply it.
                if let Some(loop_velocity) = loop_velocities.get(base_name) {
                    def.base_velocity = *loop_velocity;
                }
            }
        } // Create a set of all clip names for fast `contains` checks
        let all_clip_names_set: HashSet<String> =
            animation_definitions_map.keys().cloned().collect();

        // A temporary list of links to create, to avoid mutable borrow issues with the map.
        let mut links_to_create: Vec<(String, String)> = Vec::new();

        // Iterate over the keys of the original map to find which clips need linking.
        for name in animation_definitions_map.keys() {
            // If this is an "intro" clip (and not a loop or outro)...
            if !name.ends_with("_loop") && !name.ends_with("_outro") && !name.contains("jump") {
                let potential_loop_name = format!("{}_loop", name);
                // ...check if its corresponding loop clip exists.
                if all_clip_names_set.contains(&potential_loop_name) {
                    links_to_create.push((name.clone(), potential_loop_name));
                }
            }
            /*
            // If this is a "loop" clip...
            else if let Some(base_name) = name.strip_suffix("_loop") {
                let potential_outro_name = format!("{}_outro", base_name);
                // ...check if its corresponding outro clip exists.
                if all_clip_names_set.contains(&potential_outro_name) {
                    links_to_create.push((name.clone(), potential_outro_name));
                }
            }
            */
        }

        // Now, apply the collected links.
        for (clip_name, next_name) in links_to_create {
            if let Some(definition) = animation_definitions_map.get_mut(&clip_name) {
                definition.next_clip_name = Some(next_name);
            }
        }

        let animation_graph_handle = animation_graphs.add(animation_graph);
        commands.entity(entity).insert((
            animation_player,
            AnimationGraphHandle(animation_graph_handle),
            NifAnimator {
                skeleton_id: needs_animator_data.skeleton_id,
                animation_definitions: animation_definitions_map,
                active_animations: HashMap::new(),
                active_regions: [Priority::Idle; 4],
            },
        ));

        commands.entity(entity).remove::<NeedsNifAnimator>(); // Avoid retrying on error
        commands.trigger(NifAnimatorAdded(entity));
    }
}
fn parse_and_split_animation_blocks(nif_keys: &[TextKey]) -> Vec<ProcessedAnimation> {
    // --- Pass 1: Flatten all text key lines into a single, time-sorted list ---
    // (This pass is unchanged)
    let mut all_events: Vec<ParsedKeyEvent> = Vec::new();
    for key in nif_keys {
        for line in key.value.lines() {
            if let Some((name, command)) = parse_line(line) {
                all_events.push(ParsedKeyEvent {
                    time: key.time,
                    original_line: line.trim().to_string(),
                    name,
                    command,
                });
            }
        }
    }
    all_events.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());

    // --- Pass 2: Identify "real" animation clips ---
    // (This pass is unchanged)
    let mut clip_defining_names = std::collections::HashSet::new();
    for event in &all_events {
        if event.command == "start" || event.command == "stop" {
            clip_defining_names.insert(event.name.clone());
        }
    }

    // --- Pass 3: Determine the boundaries for only the real clips ---
    // (This pass is unchanged):
    let mut temp_blocks: HashMap<String, AnimationBlockData> = HashMap::new();
    for event in &all_events {
        let parent_clip_name = clip_defining_names
            .iter()
            .filter(|&def_name| event.name.starts_with(def_name))
            .max_by_key(|def_name| def_name.len());

        if let Some(clip_name) = parent_clip_name {
            let block =
                temp_blocks
                    .entry(clip_name.clone())
                    .or_insert_with(|| AnimationBlockData {
                        name: clip_name.clone().to_lowercase(),
                        start_time: f32::MAX,
                        end_time: f32::MIN,
                        loop_start_time: None,
                        loop_end_time: None,
                    });

            block.start_time = block.start_time.min(event.time);
            block.end_time = block.end_time.max(event.time);

            if event.name == *clip_name {
                if event.command == "loop start" {
                    block.loop_start_time = Some(event.time);
                } else if event.command == "loop stop" {
                    block.loop_end_time = Some(event.time);
                }
            }
        }
    }

    // --- Pass 4: Split clips ---
    // (This logic is also unchanged)
    let mut final_animations: Vec<ProcessedAnimation> = Vec::new();
    let temp_blocks_vec: Vec<AnimationBlockData> = temp_blocks.into_values().collect();
    for block_data in temp_blocks_vec {
        if let (Some(ls), Some(le)) = (block_data.loop_start_time, block_data.loop_end_time) {
            if le >= ls {
                if ls >= block_data.start_time {
                    final_animations.push(ProcessedAnimation {
                        name: block_data.name.clone(),
                        start_time: block_data.start_time,
                        end_time: ls,
                        events: Vec::new(),
                    });
                }
                final_animations.push(ProcessedAnimation {
                    name: format!("{}_loop", block_data.name),
                    start_time: ls,
                    end_time: le,
                    events: Vec::new(),
                });
                if block_data.end_time > le {
                    final_animations.push(ProcessedAnimation {
                        name: format!("{}_outro", block_data.name),
                        start_time: le,
                        end_time: block_data.end_time,
                        events: Vec::new(),
                    });
                }
                continue;
            }
        }
        final_animations.push(ProcessedAnimation {
            name: block_data.name,
            start_time: block_data.start_time,
            end_time: block_data.end_time,
            events: Vec::new(),
        });
    }

    // --- Pass 5 (Corrected): Populate events with Hybrid Logic ---
    // This replaces the old Pass 5 and 6. It attempts to place events by name first,
    // then falls back to placing them by time, ensuring no events are dropped.
    for event in &all_events {
        // Skip boundary markers, as they are not gameplay events.
        if event.command == "start"
            || event.command == "stop"
            || event.command == "loop start"
            || event.command == "loop stop"
        {
            continue;
        }

        let mut event_placed = false;

        // Stage 1: Try to find a parent clip by name.
        if let Some(parent_clip_name) = clip_defining_names
            .iter()
            .filter(|&def_name| event.name.starts_with(def_name))
            .max_by_key(|def_name| def_name.len())
        {
            for anim in &mut final_animations {
                let anim_base_name = anim
                    .name
                    .trim_end_matches("_loop")
                    .trim_end_matches("_outro");

                if anim_base_name == parent_clip_name.as_str()
                    && event.time >= anim.start_time - 1e-4
                    && event.time <= anim.end_time + 1e-4
                {
                    let event_string = format!("'{}' @ {:.3}", event.original_line, event.time);
                    anim.events.push(event_string);
                    event_placed = true;
                    break;
                }
            }
        }

        // Stage 2: Fallback for events with no name-based parent (e.g., "SoundGen").
        if !event_placed {
            for anim in &mut final_animations {
                if event.time >= anim.start_time - 1e-4 && event.time <= anim.end_time + 1e-4 {
                    let event_string = format!("'{}' @ {:.3}", event.original_line, event.time);
                    anim.events.push(event_string);
                    break;
                }
            }
        }
    }

    // Final sorting for consistency.
    for anim in &mut final_animations {
        anim.events.sort();
    }

    final_animations.sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
    final_animations
}
fn parse_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    if let Some(pos) = line.rfind(char::is_whitespace) {
        let mut command = line[pos..].trim().to_lowercase();
        let mut name_part = line[..pos].trim();

        // Check if the word before the command is "Loop"
        if (command == "start" || command == "stop") && name_part.to_lowercase().ends_with(" loop")
        {
            command = format!("loop {}", command);
            name_part = &name_part[..name_part.len() - 5];
        }

        let final_name = name_part.trim_end_matches(':').trim().to_string();

        if !final_name.is_empty() && !command.is_empty() {
            return Some((final_name, command));
        }
    }

    // Fallback for single-name:command pairs like "SoundGen:Left"
    if let Some(pos) = line.find(':') {
        let name = line[..pos].trim().to_string();
        let command = line[pos + 1..].trim().to_lowercase();
        if !name.is_empty() && !command.is_empty() {
            return Some((name, command));
        }
    }

    None
}
