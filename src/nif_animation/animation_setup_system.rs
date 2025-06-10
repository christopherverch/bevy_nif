// src/nif_animation/test_nif_parsing.rs

use bevy::prelude::*;
use bevy_animation::{AnimationTarget, AnimationTargetId, animated_field};
use std::collections::HashMap;

use crate::{
    NiKeyframeController,
    extra_data::ExtraFields,
    nif::{
        loader::Nif,
        spawner::NeedsNifAnimator,
        types::{NiTextKeyExtraData, ParsedBlock, TextKey},
    },
    nif_animation::{
        AnimationDefinition, BlendMask, NifAnimator, NifAnimatorAdded, REGION_ROOT_LOWER_BODY,
        parser_helpers::{
            as_text_key_extra_data, determine_bone_primary_region_index,
            filter_and_retime_keyframes, get_block, make_bevy_curve,
        },
    },
};

use super::SkeletonMap;

const REGION_INDEX_LOWER_BODY: usize = 0;
const REGION_INDEX_TORSO: usize = 1;
const REGION_INDEX_LEFT_ARM: usize = 2;
const REGION_INDEX_RIGHT_ARM: usize = 3;

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
        let global_nif_text_keys: &[TextKey] = {
            let mut root_node_index_opt: Option<usize> = None;
            for (i, block) in nif_asset.raw_parsed.blocks.iter().enumerate() {
                if let ParsedBlock::Node(node) = block {
                    if node.av_base.net_base.name.eq_ignore_ascii_case("Bip01")
                        || node.av_base.net_base.name.eq_ignore_ascii_case("Root Bone")
                    {
                        let mut current_extra_link = node.av_base.net_base.extra_data_link;
                        while let Some(extra_idx) = current_extra_link {
                            if let Some(ParsedBlock::TextKeyExtraData(_)) =
                                nif_asset.raw_parsed.blocks.get(extra_idx)
                            {
                                root_node_index_opt = Some(i);
                                break;
                            }
                            if let Some(extra_block_base) =
                                get_block(&nif_asset.raw_parsed, Some(extra_idx), |b| match b {
                                    ParsedBlock::TextKeyExtraData(tked) => {
                                        Some(&tked.extra_base as &ExtraFields)
                                    }
                                    ParsedBlock::StringExtraData(sed) => {
                                        Some(&sed.extra_base as &ExtraFields)
                                    }
                                    _ => None,
                                })
                            {
                                current_extra_link = extra_block_base.next_extra_data_link;
                            } else {
                                break;
                            }
                        }
                    }
                    if root_node_index_opt.is_some() {
                        break;
                    }
                }
            }
            if let Some(rni) = root_node_index_opt {
                if let Some(ParsedBlock::Node(n)) = nif_asset.raw_parsed.blocks.get(rni) {
                    let mut tk_data: Option<&NiTextKeyExtraData> = None;
                    let mut cel = n.av_base.net_base.extra_data_link;
                    while let Some(ei) = cel {
                        if let Some(tked) =
                            get_block(&nif_asset.raw_parsed, Some(ei), as_text_key_extra_data)
                        {
                            tk_data = Some(tked);
                            break;
                        }
                        if let Some(eb) = get_block(&nif_asset.raw_parsed, Some(ei), |b| match b {
                            ParsedBlock::TextKeyExtraData(tked) => {
                                Some(&tked.extra_base as &ExtraFields)
                            }
                            ParsedBlock::StringExtraData(sed) => {
                                Some(&sed.extra_base as &ExtraFields)
                            }
                            _ => None,
                        }) {
                            cel = eb.next_extra_data_link;
                        } else {
                            break;
                        }
                    }
                    tk_data.map_or(&[] as &[_], |data| &data.text_keys)
                } else {
                    &[]
                }
            } else {
                &[]
            }
        };

        if global_nif_text_keys.is_empty() {
            commands.entity(entity).remove::<NeedsNifAnimator>();
            continue;
        }
        let mut tagged_bones = std::collections::HashSet::new();

        // --- Call the Corrected, Optimized Parser ---
        let processed_animations = parse_and_split_animation_blocks(global_nif_text_keys);

        // --- FINAL STEP: Process Data into Curves ---
        info!(
            "--- Building Curves for {:?} (Entity {:?}) ---",
            nif_handle.path(),
            entity
        );

        info!(
            "--- Assembling Final AnimationClips for {:?} (Entity {:?}) ---",
            nif_handle.path(),
            entity
        );

        let animation_player = AnimationPlayer::default();
        let mut animation_graph = AnimationGraph::new();
        let mut animation_definitions_map: HashMap<String, AnimationDefinition> = HashMap::new();
        let mut bone_to_region_index_map: HashMap<String, usize> = HashMap::new(); // For the final component
        let root_blend_node = animation_graph.add_blend(0.5, animation_graph.root);

        for processed_clip in &processed_animations {
            let initial_bip01_pos_for_clip: Option<Vec3> = 'bip01_init_pos: {
                // Find the Bip01 controller(s)
                if let Some((_, bip01_controllers)) =
                    all_bone_controllers.iter().find(|(idx, _)| {
                        if let Some(ParsedBlock::Node(node)) =
                            nif_asset.raw_parsed.blocks.get(**idx)
                        {
                            node.av_base
                                .net_base
                                .name
                                .eq_ignore_ascii_case(REGION_ROOT_LOWER_BODY)
                        } else {
                            false
                        }
                    })
                {
                    let mut first_key: Option<(f32, Vec3)> = None;
                    for controller in bip01_controllers {
                        if let Some(kfd_idx) = controller.keyframe_data {
                            if let Some(ParsedBlock::KeyframeData(kfd)) =
                                nif_asset.raw_parsed.blocks.get(kfd_idx)
                            {
                                for key in &kfd.translations {
                                    // Check if the key is within the current clip's time range
                                    if key.time >= processed_clip.start_time - 1e-4
                                        && key.time <= processed_clip.end_time + 1e-4
                                    {
                                        // If this key is the first one we've found, or is earlier than the one we have, store it.
                                        if first_key.map_or(true, |(t, _)| key.time < t) {
                                            first_key = Some((key.time, key.value));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // If we found a key, break the labeled block and return its value
                    if let Some((_, pos)) = first_key {
                        break 'bip01_init_pos Some(pos);
                    }
                }
                // If no key or no Bip01 controller was found
                None
            };
            let mut bevy_clip = AnimationClip::default();
            let mut affected_bone_names = Vec::new();

            for (bone_block_index, controllers) in &all_bone_controllers {
                let Some(ParsedBlock::Node(bone_node)) =
                    nif_asset.raw_parsed.blocks.get(*bone_block_index)
                else {
                    continue;
                };
                let bone_name = &bone_node.av_base.net_base.name;
                let is_bip01 = bone_name.eq_ignore_ascii_case(REGION_ROOT_LOWER_BODY);
                // Get the bone's entity from the skeleton map
                let Some(bone_data) = skeleton.get_bone_by_name(bone_name) else {
                    continue;
                };
                let bone_entity = bone_data.entity;

                let mut has_curves_for_this_bone = false;
                let target_id = AnimationTargetId::from_name(&Name::new(bone_name.clone()));

                for controller in controllers {
                    let Some(keyframe_data_index) = controller.keyframe_data else {
                        continue;
                    };
                    let Some(ParsedBlock::KeyframeData(keyframe_data)) =
                        nif_asset.raw_parsed.blocks.get(keyframe_data_index)
                    else {
                        continue;
                    };

                    // --- Handle Quaternion Rotations ---
                    if !keyframe_data.quaternion_keys.is_empty() {
                        let raw_keys: Vec<(f32, Quat)> = keyframe_data
                            .quaternion_keys
                            .iter()
                            .map(|k| (k.time, k.value))
                            .collect();
                        let rot_keys = filter_and_retime_keyframes(
                            &raw_keys,
                            processed_clip.start_time,
                            processed_clip.end_time,
                        );
                        if let Some(curve) = make_bevy_curve(&rot_keys) {
                            bevy_clip.add_curve_to_target(
                                target_id,
                                AnimatableCurve::new(animated_field!(Transform::rotation), curve),
                            );
                            has_curves_for_this_bone = true;
                        }
                    }

                    // --- Handle Translations ---
                    if !keyframe_data.translations.is_empty() {
                        let raw_keys: Vec<(f32, Vec3)> = if is_bip01 {
                            if let Some(initial_pos) = initial_bip01_pos_for_clip {
                                keyframe_data
                                    .translations
                                    .iter()
                                    .map(|k| {
                                        (k.time, Vec3::new(initial_pos.x, initial_pos.y, k.value.z))
                                    })
                                    .collect()
                            } else {
                                keyframe_data
                                    .translations
                                    .iter()
                                    .map(|k| (k.time, k.value))
                                    .collect()
                            }
                        } else {
                            keyframe_data
                                .translations
                                .iter()
                                .map(|k| (k.time, k.value))
                                .collect()
                        };

                        let trans_keys = filter_and_retime_keyframes(
                            &raw_keys,
                            processed_clip.start_time,
                            processed_clip.end_time,
                        );
                        if let Some(curve) = make_bevy_curve(&trans_keys) {
                            bevy_clip.add_curve_to_target(
                                target_id,
                                AnimatableCurve::new(
                                    animated_field!(Transform::translation),
                                    curve,
                                ),
                            );
                            has_curves_for_this_bone = true;
                        }
                    }
                }

                if has_curves_for_this_bone {
                    affected_bone_names.push(bone_name.clone());
                    // If we haven't tagged this bone yet, insert the AnimationTarget component.
                    if tagged_bones.insert(bone_entity) {
                        commands.entity(bone_entity).insert(AnimationTarget {
                            id: target_id,
                            player: entity, // Link back to the root entity with the AnimationPlayer
                        });
                        if let Some(skeleton_data) = skeleton_map_res
                            .skeletons
                            .get(&needs_animator_data.skeleton_id)
                        {
                            let region_idx =
                                determine_bone_primary_region_index(&bone_data.name, skeleton_data);
                            animation_graph.add_target_to_mask_group(target_id, region_idx as u32);
                        }
                    }
                }
            }
            let bevy_clip_handle = bevy_animation_clips.add(bevy_clip);
            let graph_node_index =
                animation_graph.add_clip(bevy_clip_handle.clone(), 1.0, root_blend_node);

            // --- Calculate Blend Mask ---
            let mut inherent_mask = BlendMask::empty(); // Use the type-safe bitflag

            for bone_name in &affected_bone_names {
                // This is placeholder logic for mapping a bone to a body region.
                // A real implementation would use the skeleton hierarchy.
                let region_idx = if bone_name.contains("Spine")
                    || bone_name.contains("Torso")
                    || bone_name.contains("Neck")
                    || bone_name.contains("Head")
                {
                    REGION_INDEX_TORSO
                } else if bone_name.contains(" L ") {
                    REGION_INDEX_LEFT_ARM
                } else if bone_name.contains(" R ") {
                    REGION_INDEX_RIGHT_ARM
                } else {
                    REGION_INDEX_LOWER_BODY
                };
                bone_to_region_index_map.insert(bone_name.clone(), region_idx);

                // Convert the region index back to the correct flag and insert it into the mask
                let flag = match region_idx {
                    REGION_INDEX_LOWER_BODY => BlendMask::LOWER_BODY,
                    REGION_INDEX_TORSO => BlendMask::TORSO,
                    REGION_INDEX_LEFT_ARM => BlendMask::LEFT_ARM,
                    REGION_INDEX_RIGHT_ARM => BlendMask::RIGHT_ARM,
                    _ => BlendMask::empty(),
                };
                inherent_mask.insert(flag);
            }

            animation_definitions_map.insert(
                processed_clip.name.clone(),
                AnimationDefinition {
                    node_index: graph_node_index,
                    inherent_mask, // Assuming your struct takes the mask directly
                },
            );
            info!(
                "  -> BUILT CLIP: '{}', affecting {} bones. Blend Mask: {:#010b}, {}  {}",
                processed_clip.name, // Use the name from our own struct
                affected_bone_names.len(),
                inherent_mask,
                processed_clip.start_time,
                processed_clip.end_time,
            );
        }
        let animation_graph_handle = animation_graphs.add(animation_graph);
        commands.entity(entity).insert((
            animation_player,
            AnimationGraphHandle(animation_graph_handle),
            NifAnimator {
                skeleton_id: needs_animator_data.skeleton_id,
                animation_definitions: animation_definitions_map,
                bone_to_region_index_map,
                active_animations: HashMap::new(),
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
        println!("{:?}", key);
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
    let mut clip_defining_names = std::collections::HashSet::new();
    for event in &all_events {
        if event.command == "start" || event.command == "stop" {
            clip_defining_names.insert(event.name.clone());
        }
    }

    // --- Pass 3: Determine the boundaries for only the real clips ---
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
                        name: clip_name.clone(),
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

    // --- Pass 4: Split clips and populate with ALL relevant events ---
    let mut final_animations: Vec<ProcessedAnimation> = Vec::new();
    let temp_blocks_vec: Vec<AnimationBlockData> = temp_blocks.into_values().collect();
    for block_data in temp_blocks_vec {
        if let (Some(ls), Some(le)) = (block_data.loop_start_time, block_data.loop_end_time) {
            if le > ls {
                if ls > block_data.start_time {
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

    // Correctly populate events into the final, split clips
    for anim in &mut final_animations {
        let base_name = anim
            .name
            .trim_end_matches("_loop")
            .trim_end_matches("_outro");
        for event in &all_events {
            if event.time >= anim.start_time && event.time <= anim.end_time {
                // CORRECTED LOGIC: Check if the event's name starts with the clip's base name.
                if event.name.starts_with(base_name) {
                    // Don't add the boundary markers themselves as "events"
                    if event.command != "start"
                        && event.command != "stop"
                        && event.command != "loop start"
                        && event.command != "loop stop"
                    {
                        let event_string = format!("'{}' @ {:.3}", event.original_line, event.time);
                        anim.events.push(event_string);
                    }
                }
            }
        }
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
