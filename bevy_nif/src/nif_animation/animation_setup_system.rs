use crate::{
    loader::Nif,
    nif_animation::{
        AnimationDefinition, NifAnimator, NifAnimatorAdded, NifEvent, NifEventType,
        REGION_ROOT_LOWER_BODY,
        bevy_types::Priority,
        parser_helpers::{
            determine_bone_primary_region_index, filter_and_retime_keyframes,
            is_inherently_looping, make_bevy_curve, sample_vec3_curve,
        },
    },
    spawner::NeedsNifAnimator,
};
use bevy::{
    animation::{AnimationTarget, AnimationTargetId, animated_field},
    prelude::*,
};
use nif::{NiKeyframeController, NiTextKey, loader::NiKey};
use slotmap::Key;
use std::collections::{HashMap, HashSet};

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
    pub min_attack_time_relative: f32,
    pub max_attack_time_relative: f32,
    pub hit_time_relative: f32,
    pub min_hit_time_relative: f32,
}

/// A temporary struct to hold all data about a block before the final split decision.
#[derive(Debug, Clone)]
struct AnimationBlockData {
    name: String,
    start_time: f32,
    end_time: f32,
    loop_start_time: Option<f32>,
    loop_end_time: Option<f32>,
    min_attack_time: Option<f32>,
    max_attack_time: Option<f32>,
    hit_time: Option<f32>,
    min_hit_time: Option<f32>,
    follow_events: HashMap<String, f32>,
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
        let mut all_bone_controllers: HashMap<NiKey, Vec<NiKeyframeController>> = HashMap::new();
        for (target_key, kfc) in &nif_asset.all_controller_links {
            all_bone_controllers
                .entry(*target_key)
                .or_default()
                .push(kfc.clone());
        }
        let global_nif_text_keys: &[NiTextKey] = &nif_asset.text_keys;
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

        for (bone_key, controllers) in &all_bone_controllers {
            let Some(bone_name) = nif_asset.node_names.get(bone_key) else {
                continue;
            };
            let Some(bone_data) = skeleton.get_bone_by_name(bone_name) else {
                continue;
            };

            let mut bone_anim = RawBoneAnimation {
                target_id: AnimationTargetId::from_name(&Name::new(bone_name.clone())),
                bone_entity: bone_data.entity,
                bone_name: bone_name.to_string(),
                is_bip01: bone_name.eq_ignore_ascii_case(REGION_ROOT_LOWER_BODY),
                all_rotation_keys: Vec::new(),
                all_translation_keys: Vec::new(),
            };

            for controller in controllers {
                let keyframe_data_key = controller.data.key;
                if keyframe_data_key.is_null() {
                    continue;
                }
                let Some(keyframe_data) = nif_asset.all_keyframe_data.get(&keyframe_data_key)
                else {
                    continue;
                };

                // ----------------------------------------------------------------------
                // ROTATION KEY EXTRACTION (Quaternions)
                // ----------------------------------------------------------------------
                let rot_keys_enum = &keyframe_data.rotations.keys;

                match rot_keys_enum {
                    nif::NiRotKey::LinKey(linear_keys) => {
                        bone_anim
                            .all_rotation_keys
                            .extend(linear_keys.iter().map(|k| (k.time, k.value)));
                    }
                    nif::NiRotKey::BezKey(bezier_keys) => {
                        bone_anim
                            .all_rotation_keys
                            .extend(bezier_keys.iter().map(|k| (k.time, k.value)));
                    }
                    nif::NiRotKey::TCBKey(tcb_keys) => {
                        bone_anim
                            .all_rotation_keys
                            .extend(tcb_keys.iter().map(|k| (k.time, k.value)));
                    }
                    nif::NiRotKey::EulerKey(_) => {
                        // Skip Euler key handling, as it requires complex conversion
                        warn!(
                            "Skipping NiEulerRotKeys for bone {} as complex conversion is required.",
                            bone_anim.bone_name
                        );
                    }
                }

                // ----------------------------------------------------------------------
                // TRANSLATION KEY EXTRACTION (Vec3 Positions)
                // ----------------------------------------------------------------------
                let pos_keys_enum = &keyframe_data.translations.keys;

                match pos_keys_enum {
                    nif::NiPosKey::LinKey(linear_keys) => {
                        bone_anim
                            .all_translation_keys
                            .extend(linear_keys.iter().map(|k| (k.time, k.value)));
                    }
                    nif::NiPosKey::BezKey(bezier_keys) => {
                        bone_anim
                            .all_translation_keys
                            .extend(bezier_keys.iter().map(|k| (k.time, k.value)));
                    }
                    nif::NiPosKey::TCBKey(tcb_keys) => {
                        bone_anim
                            .all_translation_keys
                            .extend(tcb_keys.iter().map(|k| (k.time, k.value)));
                    }
                }
            }

            // If we found any keys for this bone, sort them by time and store them.
            if !bone_anim.all_rotation_keys.is_empty() || !bone_anim.all_translation_keys.is_empty()
            {
                // Sorting here is needed for the filtering logic later.
                bone_anim
                    .all_rotation_keys
                    .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                bone_anim
                    .all_translation_keys
                    .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

                // Tag the bone with AnimationTarget during extraction.
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
            let bip01_target_key = all_bone_controllers.keys().find(|bone_key| {
                nif_asset
                    .node_names
                    .get(*bone_key)
                    .map(|name| name.eq_ignore_ascii_case(REGION_ROOT_LOWER_BODY))
                    .unwrap_or(false)
            });

            let Some(bone_key) = bip01_target_key else {
                // If there's no Bip01 controller, break with an empty Vec.
                dbg!("no bip_01");
                break 'find_keys Vec::new();
            };

            // Get the controllers associated with that bone key.
            let Some(bip01_controllers) = all_bone_controllers.get(bone_key) else {
                dbg!("no bip_01_controller");
                break 'find_keys Vec::new();
            };

            let mut keys = Vec::new();
            for controller in bip01_controllers {
                let keyframe_data_key = controller.data.key;
                dbg!(keyframe_data_key);
                dbg!("no kfd");
                if keyframe_data_key.is_null() {
                    continue;
                }

                if let Some(kfd) = nif_asset.all_keyframe_data.get(&keyframe_data_key) {
                    // FIX: Apply NiPosKey pattern matching for translation data, same as above.
                    let pos_keys_enum = &kfd.translations.keys;

                    match pos_keys_enum {
                        nif::NiPosKey::LinKey(linear_keys) => {
                            for key in linear_keys {
                                keys.push((key.time, key.value));
                            }
                        }
                        nif::NiPosKey::BezKey(bezier_keys) => {
                            for key in bezier_keys {
                                keys.push((key.time, key.value));
                            }
                        }
                        nif::NiPosKey::TCBKey(tcb_keys) => {
                            for key in tcb_keys {
                                keys.push((key.time, key.value));
                            }
                        }
                    }
                }
            }

            // Sort the keys by time for the binary search.
            keys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            keys
        };
        let animation_player = AnimationPlayer::default();
        let mut animation_graph = AnimationGraph::new();
        let mut animation_definitions_map: HashMap<String, AnimationDefinition> = HashMap::new();
        let root_blend_node = animation_graph.add_blend(0.5, animation_graph.root);

        // Add targets to graph mask based on pre-filtered `raw_bone_data`
        for bone in &raw_bone_data {
            let region_idx = determine_bone_primary_region_index(&bone.bone_name, skeleton);
            animation_graph.add_target_to_mask_group(bone.target_id, region_idx as u32);
        }

        for processed_clip in &processed_animations {
            let mut root_translation_curve: Option<AnimatableKeyframeCurve<Vec3>> = None;
            let mut bevy_clip = AnimationClip::default();

            // Loop over our pre-collected bone data
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
                    // Get the correctly timed keys for the current clip segment.
                    let trans_keys = filter_and_retime_keyframes(
                        bone.all_translation_keys.iter().cloned(),
                        processed_clip.start_time,
                        processed_clip.end_time,
                    );

                    if let Some(curve) = make_bevy_curve(&trans_keys) {
                        if bone.is_bip01 {
                            // If it's the root bone, store its translation curve in our temporary variable.
                            root_translation_curve = Some(curve);
                        } else {
                            // For any other bone, add its translation to the main animation clip as before.
                            bevy_clip.add_curve_to_target(
                                bone.target_id,
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

            // Use the helper function to check if this clip should define a velocity.
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
                    min_attack_time_relative: processed_clip.min_attack_time_relative,
                    hit_time_relative: processed_clip.hit_time_relative,
                    min_hit_time_relative: processed_clip.min_hit_time_relative,
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
fn parse_and_split_animation_blocks(nif_keys: &[NiTextKey]) -> Vec<ProcessedAnimation> {
    // --- Pass 1: Flatten all text key lines into a single, time-sorted list ---
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
                        name: clip_name.clone().to_lowercase(),
                        start_time: f32::MAX,
                        end_time: f32::MIN,
                        loop_start_time: None,
                        loop_end_time: None,
                        min_attack_time: None,
                        max_attack_time: None,
                        hit_time: None,
                        min_hit_time: None,
                        follow_events: HashMap::new(),
                    });

            block.start_time = block.start_time.min(event.time);
            block.end_time = block.end_time.max(event.time);

            match event.command.as_str() {
                "loop start" => block.loop_start_time = Some(event.time),
                "loop stop" => block.loop_end_time = Some(event.time),
                "min attack" => block.min_attack_time = Some(event.time),
                "max attack" => block.max_attack_time = Some(event.time),
                "hit" => block.hit_time = Some(event.time),
                "min hit" => block.min_hit_time = Some(event.time),
                cmd if cmd.contains(" follow") && cmd.ends_with(" start") => {
                    // We want to store the base name, e.g., "large follow", not "large follow start".
                    if let Some(base_cmd) = cmd.strip_suffix(" start") {
                        block
                            .follow_events
                            .insert(base_cmd.trim().to_string(), event.time);
                    }
                }
                _ => {} // Ignore other commands like "start", "stop" here
            }
        }
    }

    // --- Pass 4: Split clips ---
    let mut final_animations: Vec<ProcessedAnimation> = Vec::new();
    let temp_blocks_vec: Vec<AnimationBlockData> = temp_blocks.into_values().collect();

    for block_data in temp_blocks_vec {
        // First, check for looping animations (your existing logic is correct)
        if let (Some(ls), Some(le)) = (block_data.loop_start_time, block_data.loop_end_time) {
            if le >= ls {
                if ls > block_data.start_time {
                    final_animations.push(ProcessedAnimation {
                        name: block_data.name.clone(),
                        start_time: block_data.start_time,
                        end_time: ls,
                        events: Vec::new(),
                        min_attack_time_relative: 0.0,
                        max_attack_time_relative: 0.0,
                        hit_time_relative: 0.0,
                        min_hit_time_relative: 0.0,
                    });
                }
                final_animations.push(ProcessedAnimation {
                    name: format!("{}_loop", block_data.name),
                    start_time: ls,
                    end_time: le,
                    events: Vec::new(),
                    min_attack_time_relative: 0.0,
                    max_attack_time_relative: 0.0,
                    hit_time_relative: 0.0,
                    min_hit_time_relative: 0.0,
                });
                if block_data.end_time > le {
                    final_animations.push(ProcessedAnimation {
                        name: format!("{}_outro", block_data.name),
                        start_time: le,
                        end_time: block_data.end_time,
                        events: Vec::new(),
                        min_attack_time_relative: 0.0,
                        max_attack_time_relative: 0.0,
                        hit_time_relative: 0.0,
                        min_hit_time_relative: 0.0,
                    });
                }
                continue;
            }
        } else if block_data.min_attack_time.is_some() || !block_data.follow_events.is_empty() {
            let base_name = &block_data.name;

            // --- 1. Create Windup Clip (if it exists) ---
            let release_start_time = if let (Some(min_attack), Some(max_attack)) =
                (block_data.min_attack_time, block_data.max_attack_time)
            {
                final_animations.push(ProcessedAnimation {
                    name: format!("{}_windup", base_name),
                    start_time: block_data.start_time,
                    end_time: max_attack,
                    events: Vec::new(),
                    min_attack_time_relative: min_attack - block_data.start_time,
                    max_attack_time_relative: max_attack - block_data.start_time,
                    hit_time_relative: 0.0,
                    min_hit_time_relative: 0.0,
                });
                max_attack
            } else {
                block_data.start_time
            };

            // --- 2. Create Release/Shoot Clip ---
            let mut sorted_follows: Vec<_> = block_data.follow_events.iter().collect();
            sorted_follows.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap());

            let release_end_time = if let Some(hit_time) = block_data.hit_time {
                hit_time
            } else if let Some((_, first_follow_time)) = sorted_follows.first() {
                **first_follow_time
            } else {
                block_data.end_time
            };

            // Calculate relative times and add them to the _release clip.
            let mut hit_time_rel = 0.0;
            let mut min_hit_time_rel = 0.0;
            if let Some(hit_time) = block_data.hit_time {
                hit_time_rel = hit_time - release_start_time;
                if let Some(min_hit) = block_data.min_hit_time {
                    if min_hit < hit_time {
                        min_hit_time_rel = min_hit - release_start_time;
                    }
                }
            }

            final_animations.push(ProcessedAnimation {
                name: format!("{}_release", base_name),
                start_time: release_start_time,
                end_time: release_end_time,
                events: Vec::new(),
                min_attack_time_relative: 0.0,
                max_attack_time_relative: 0.0,
                hit_time_relative: hit_time_rel, // Add calculated value
                min_hit_time_relative: min_hit_time_rel, // Add calculated value
            });

            // --- 3. Create a clip for EACH specific follow event ---
            if !sorted_follows.is_empty() {
                // If the animation continues after the release but before the first follow,
                // create a generic follow/recovery clip for that duration.
                if sorted_follows.first().unwrap().1 > &release_end_time {
                    final_animations.push(ProcessedAnimation {
                        name: format!("{}_follow", base_name),
                        start_time: release_end_time,
                        end_time: *sorted_follows.first().unwrap().1, // End at the start of the first named follow
                        events: Vec::new(),
                        min_attack_time_relative: 0.0,
                        max_attack_time_relative: 0.0,
                        hit_time_relative: 0.0,
                        min_hit_time_relative: 0.0,
                    });
                }

                // Create the specific follow clips
                for (command, start_time) in &sorted_follows {
                    let follow_type_name = command.replace(' ', "_"); // e.g., "large follow" -> "large_follow"

                    // Find the correct "stop" time for this specific follow clip.
                    let stop_event_cmd = format!("{} stop", command); // e.g., "large follow stop"

                    // Search all_events (from Pass 1) for the matching stop command.
                    let stop_time = all_events
                        .iter()
                        .find(|event| {
                            event.name.to_lowercase() == block_data.name
                                && event.command == stop_event_cmd
                        })
                        .map_or(
                            block_data.end_time, // Fallback to the main block's end time if not found.
                            |event| event.time,  // Use the time from the found "follow stop" event.
                        );

                    final_animations.push(ProcessedAnimation {
                        name: format!("{}_{}", base_name, follow_type_name),
                        start_time: **start_time,
                        end_time: stop_time, // Use the correct, specific stop time.
                        events: Vec::new(),
                        min_attack_time_relative: 0.0,
                        max_attack_time_relative: 0.0,
                        hit_time_relative: 0.0,
                        min_hit_time_relative: 0.0,
                    });
                }
            } else if block_data.end_time > release_end_time + 1e-4 {
                // This handles melee attacks that have a follow-through but no named follow events.
                final_animations.push(ProcessedAnimation {
                    name: format!("{}_follow", base_name),
                    start_time: release_end_time,
                    end_time: block_data.end_time,
                    events: Vec::new(),
                    min_attack_time_relative: 0.0,
                    max_attack_time_relative: 0.0,
                    hit_time_relative: 0.0,
                    min_hit_time_relative: 0.0,
                });
            }

            continue; // Go to the next animation block
        }
        // If it's not a looping anim or a main attack anim, treat it as a single clip.
        // This will correctly handle animations like "equip", "unequip", "hit", "death",
        // and the separate "follow" animations.
        final_animations.push(ProcessedAnimation {
            name: block_data.name,
            start_time: block_data.start_time,
            end_time: block_data.end_time,
            events: Vec::new(),
            min_attack_time_relative: 0.0,
            max_attack_time_relative: 0.0,
            hit_time_relative: 0.0,
            min_hit_time_relative: 0.0,
        });
    }
    // --- Pass 5: Populate events with Hybrid Logic ---
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
// A list of all possible commands we need to recognize.
// Order is important for commands that are subsets of others (e.g., "shoot follow" before "shoot").
const KNOWN_COMMANDS: &[&str] = &[
    // Most specific commands first
    "large follow start",
    "large follow stop",
    "medium follow start",
    "medium follow stop",
    "small follow start",
    "small follow stop",
    "shoot follow start",
    "shoot follow stop",
    "follow start",
    "follow stop",
    "loop start",
    "loop stop",
    "min attack",
    "max attack",
    "min hit",
    "hit",
    // Less specific commands last
    "large follow",
    "medium follow",
    "small follow",
    "shoot follow",
    "start",
    "stop",
    "shoot",
    "block",
    "equip",
    "unequip",
];
fn parse_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let lower_line = line.to_lowercase();

    for &cmd in KNOWN_COMMANDS {
        if lower_line.ends_with(cmd) {
            let cmd_start_index = lower_line.len() - cmd.len();
            // Ensure we are matching a whole word command, not just a partial suffix.
            if cmd_start_index == 0 || lower_line.as_bytes().get(cmd_start_index - 1) == Some(&b' ')
            {
                // Trim whitespace, then remove the colon.
                let name_part = line[..cmd_start_index].trim().trim_end_matches(':').trim();
                if !name_part.is_empty() {
                    return Some((name_part.to_string(), cmd.to_string()));
                }
            }
        }
    }

    // Fallback for simple "Group:Action" lines like "SoundGen:Left"
    if let Some(pos) = line.find(':') {
        let name = line[..pos].trim().to_string();
        let command = line[pos + 1..].trim().to_lowercase();
        if !name.is_empty() && !command.is_empty() {
            return Some((name, command));
        }
    }

    None
}
