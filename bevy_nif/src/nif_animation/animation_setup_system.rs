//! Bevy-side animation setup: extract bone keyframe controllers from the
//! NIF, build one Bevy `AnimationClip` per `ProcessedAnimation` (produced by
//! `animation_block_parser::parse_and_split_animation_blocks`), wire them
//! into the animation graph, and install the `NifAnimator` on the root.

use crate::{
    loader::Nif,
    nif_animation::{
        animation_block_parser::{combat_anim_data_for, parse_and_split_animation_blocks},
        bevy_types::{AnimType, Priority},
        parser_helpers::{
            determine_bone_primary_region_index, filter_and_retime_keyframes, flatten_keys_xz,
            is_inherently_looping, make_bevy_curve, sample_vec3_curve,
        },
        AnimationDefinition, NifAnimator, NifEvent, NifEventType, REGION_ROOT_LOWER_BODY,
    },
    skeleton::Skeleton,
};
use bevy_animation::{
    AnimatedBy, AnimationClip, AnimationPlayer, AnimationTargetId, animated_field,
    animation_curves::{AnimatableCurve, AnimatableKeyframeCurve},
    graph::{AnimationGraph, AnimationGraphHandle},
};
use bevy_asset::LoadContext;
use bevy_ecs::{entity::Entity, name::Name, world::World};
use bevy_log::warn;
use bevy_math::{Quat, Vec3};
use bevy_transform::components::Transform;
use nif::{NiKey, NiKeyframeController, NiTextKey};
use slotmap::Key;
use std::collections::{HashMap, HashSet};

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
    root_entity: Entity,
    nif: &Nif,
    skeleton: &Skeleton,
    load_context: &mut LoadContext,
    world: &mut World,
) {
    // --- STEP 1: Extract Bone Controllers ---
    let mut all_bone_controllers: HashMap<NiKey, Vec<NiKeyframeController>> = HashMap::new();
    for (target_key, kfc) in &nif.all_controller_links {
        all_bone_controllers
            .entry(*target_key)
            .or_default()
            .push(kfc.clone());
    }
    let global_nif_text_keys: &[NiTextKey] = &nif.text_keys;
    if global_nif_text_keys.is_empty() {
        return;
    }
    // --- Call the Parser ---
    let processed_animations = parse_and_split_animation_blocks(global_nif_text_keys);
    let loop_base_names: HashSet<String> = processed_animations
        .iter()
        .filter_map(|clip| clip.name.strip_suffix("_loop").map(String::from))
        .collect();

    let mut raw_bone_data: Vec<RawBoneAnimation> = Vec::new();
    let mut tagged_bones = HashSet::new();

    for (bone_key, controllers) in &all_bone_controllers {
        let Some(bone_name) = nif.node_names.get(bone_key) else {
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
            let Some(keyframe_data) = nif.all_keyframe_data.get(&keyframe_data_key) else {
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
        if !bone_anim.all_rotation_keys.is_empty() || !bone_anim.all_translation_keys.is_empty() {
            // Sorting here is needed for the filtering logic later.
            bone_anim
                .all_rotation_keys
                .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            bone_anim
                .all_translation_keys
                .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

            // Tag the bone with AnimationTarget during extraction.
            if tagged_bones.insert(bone_anim.bone_entity) {
                world
                    .entity_mut(bone_anim.bone_entity)
                    .insert((bone_anim.target_id, AnimatedBy(root_entity)));
            }

            raw_bone_data.push(bone_anim);
        }
    }

    // Pre-process and sort the Bip01 translation keys once for fast lookups.
    let bip01_translation_keys: Vec<(f32, Vec3)> = 'find_keys: {
        // Find the Bip01/root bone controller index first.
        let bip01_target_key = all_bone_controllers.keys().find(|bone_key| {
            nif.node_names
                .get(*bone_key)
                .map(|name| name.eq_ignore_ascii_case(REGION_ROOT_LOWER_BODY))
                .unwrap_or(false)
        });

        let Some(bone_key) = bip01_target_key else {
            // If there's no Bip01 controller, break with an empty Vec.
            //dbg!("no bip_01");
            break 'find_keys Vec::new();
        };

        // Get the controllers associated with that bone key.
        let Some(bip01_controllers) = all_bone_controllers.get(bone_key) else {
            //dbg!("no bip_01_controller");
            break 'find_keys Vec::new();
        };

        let mut keys = Vec::new();
        for controller in bip01_controllers {
            let keyframe_data_key = controller.data.key;
            //dbg!(keyframe_data_key);
            //dbg!("no kfd");
            if keyframe_data_key.is_null() {
                continue;
            }

            if let Some(kfd) = nif.all_keyframe_data.get(&keyframe_data_key) {
                // Apply NiPosKey pattern matching for translation data
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
    // Text key events per clip name (relative time, original line) — used by
    // gameplay systems for C++ getTextKeyTime-style lookups and key enumeration.
    let mut text_key_events_map: HashMap<String, Vec<(f32, String)>> = HashMap::new();
    let root_blend_node = animation_graph.add_blend(0.5, animation_graph.root);

    // Add targets to graph mask based on pre-filtered `raw_bone_data`
    for bone in &raw_bone_data {
        let region_idx = determine_bone_primary_region_index(&bone.bone_name, skeleton);
        animation_graph.add_target_to_mask_group(bone.target_id, region_idx as u32);
    }
    for processed_clip in &processed_animations {
        let mut root_translation_curve: Option<AnimatableKeyframeCurve<Vec3>> = None;
        let mut bevy_clip = AnimationClip::default();

        // Loop over the pre-collected bone data
        for bone in &raw_bone_data {
            // --- Handle Rotations from pre-collected data ---
            if !bone.all_rotation_keys.is_empty() {
                let rot_keys = filter_and_retime_keyframes(
                    bone.all_rotation_keys.clone(), // .cloned() is cheap for tuples of primatives
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
                let raw_trans_keys = bone.all_translation_keys.clone();

                if bone.is_bip01 {
                    // Perform flattening on the raw keys first
                    let flattened_raw = flatten_keys_xz(raw_trans_keys.clone());

                    // Now Retime BOTH sets
                    let retimed_raw = filter_and_retime_keyframes(
                        raw_trans_keys,
                        processed_clip.start_time,
                        processed_clip.end_time,
                    );
                    let retimed_flattened = filter_and_retime_keyframes(
                        flattened_raw,
                        processed_clip.start_time,
                        processed_clip.end_time,
                    );

                    // root translation curve has the raw data so we can use it for movement
                    if let Some(full_curve) = make_bevy_curve(&retimed_raw) {
                        root_translation_curve = Some(full_curve);
                    }
                    // the animation only contains up-down movement
                    if let Some(flat_curve) = make_bevy_curve(&retimed_flattened) {
                        bevy_clip.add_curve_to_target(
                            bone.target_id,
                            AnimatableCurve::new(
                                animated_field!(Transform::translation),
                                flat_curve,
                            ),
                        );
                    }
                } else {
                    // Get the correctly timed keys for the current clip segment.
                    let trans_keys = filter_and_retime_keyframes(
                        raw_trans_keys,
                        processed_clip.start_time,
                        processed_clip.end_time,
                    );

                    if let Some(curve) = make_bevy_curve(&trans_keys) {
                        // For any other bone, add its translation to the main animation clip
                        bevy_clip.add_curve_to_target(
                            bone.target_id,
                            AnimatableCurve::new(animated_field!(Transform::translation), curve),
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
                        // Store the structured key so gameplay systems can query key
                        // times (C++ NIFAnimation::getTextKeyTime semantics).
                        text_key_events_map
                            .entry(processed_clip.name.clone())
                            .or_default()
                            .push((relative_time, original_line.to_string()));

                        // Emit a generic TextKey event for every key so gameplay
                        // systems can implement C++ handleTextKey dispatch.
                        bevy_clip.add_event(
                            relative_time,
                            NifEvent {
                                event_type: NifEventType::TextKey {
                                    line: original_line.to_string(),
                                },
                                entity: root_entity,
                            },
                        );

                        // Legacy SoundGen event (kept for existing consumers).
                        if original_line.to_lowercase().contains("soundgen") {
                            bevy_clip.add_event(
                                relative_time,
                                NifEvent {
                                    event_type: NifEventType::SoundGen {
                                        sound_name: original_line.to_string(),
                                    },
                                    entity: root_entity,
                                },
                            );
                        }
                    }
                }
            }
        }
        let clip_duration = bevy_clip.duration();
        let bevy_clip_handle =
            load_context.add_labeled_asset(format!("Clip_{}", processed_clip.name), bevy_clip);
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
                        base_velocity =
                            Vec3::new(displacement.x / duration, displacement.y / duration, 0.0);
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
                anim_type: processed_clip.anim_type,
                next_clip_name: None,
                duration: clip_duration,
                base_velocity,
                root_translation_curve,
                combat_anim_data: combat_anim_data_for(processed_clip),
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
    let all_clip_names_set: HashSet<String> = animation_definitions_map.keys().cloned().collect();

    // A temporary list of links to create, to avoid mutable borrow issues with the map.
    let mut outro_links_to_create: Vec<(String, String)> = Vec::new();
    let mut loop_links_to_create: Vec<(String, String)> = Vec::new();

    // Iterate over the keys of the original map to find which clips need linking.
    for name in animation_definitions_map.keys() {
        if name.contains("knockout") {
            //dbg!(name);
        }
        // If this is an "intro" clip (and not a loop or outro)...
        if !name.ends_with("_loop") && !name.ends_with("_outro") {
            let potential_loop_name = format!("{}_loop", name);
            // ...check if its corresponding loop clip exists.
            if all_clip_names_set.contains(&potential_loop_name) {
                loop_links_to_create.push((name.clone(), potential_loop_name));
            }
        }
        // If this is a "loop" clip...
        else if let Some(base_name) = name.strip_suffix("_loop") {
            let potential_outro_name = format!("{}_outro", base_name);
            if base_name.contains("knockout") {
                //dbg!(&potential_outro_name);
            }
            // ...check if its corresponding outro clip exists.
            if all_clip_names_set.contains(&potential_outro_name) {
                outro_links_to_create.push((name.clone(), potential_outro_name));
            }
        }
    }

    // Now, apply the collected links.
    for (clip_name, next_name) in loop_links_to_create {
        let mut base_velocity = Vec3::ZERO;
        if let Some(loop_definition) = animation_definitions_map.get(&next_name) {
            base_velocity = loop_definition.base_velocity;
        }
        if let Some(definition) = animation_definitions_map.get_mut(&clip_name) {
            definition.next_clip_name = Some(next_name);
            // We do this here instead of in the animation setup, since we don't know what is an
            // intro until it's assigned a loop as its next clip
            definition.anim_type = AnimType::Intro;
            definition.base_velocity = base_velocity;
        }
    }
    for (clip_name, next_name) in outro_links_to_create {
        if let Some(definition) = animation_definitions_map.get_mut(&clip_name) {
            definition.next_clip_name = Some(next_name);
        }
    }

    let animation_graph_handle =
        load_context.add_labeled_asset("AnimationGraph".to_string(), animation_graph);
    world.entity_mut(root_entity).insert((
        animation_player,
        AnimationGraphHandle(animation_graph_handle),
        NifAnimator {
            animation_definitions: animation_definitions_map,
            active_animations: HashMap::new(),
            active_regions: [Priority::Default; 4],
            text_key_events: text_key_events_map,
        },
    ));
}
