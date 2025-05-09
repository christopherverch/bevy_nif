// --- Necessary Imports ---

// --- Bevy Imports (Corrected for Bevy 0.16+) ---
use bevy::animation::AnimationClip;
use bevy::animation::AnimationTargetId;
use bevy::asset::{Assets, Handle}; // Keep Assets, Handle
use bevy::ecs::entity::Entity;
use bevy::math::curve::interval::InvalidIntervalError;
use bevy::prelude::*;
// use bevy::log::{error, info, warn};
use bevy::math::{Quat, Vec3};
use bevy::prelude::{
    Commands,
    Name,
    Query,
    Res,
    ResMut,
    Resource, // Keep needed prelude items
};
use bevy_animation::AnimationTarget;
use bevy_animation::animated_field;
use std::collections::HashMap;

use crate::NiKeyframeController;
use crate::NiKeyframeData;
use crate::NiTextKeyExtraData;
use crate::ParsedBlock;
use crate::ParsedNifData;
use crate::RecordLink;
use crate::extra_data::ExtraFields;
use crate::modular_characters::NeedsNifAnimator;

use super::loader::Nif; // Needed for add_curve_boxed potentially, or boxing

// --- Assume your Key structs, KeyType, NiKeyframeData, NiKeyframeController ---
// --- ParsedNifData, RecordLink, Vector3 etc. are defined exactly as you provided ---

// --- Intermediate representation for an animation curve for a specific bone ---
#[derive(Debug, Clone)]
pub struct BoneAnimationCurve {
    pub target_bone_name: String,                // Name of the NiNode (bone)
    pub rotations: Vec<(f32, bevy::math::Quat)>, // (time, rotation_value)
    pub translations: Vec<(f32, bevy::math::Vec3)>, // (time, translation_value)
    pub scales: Vec<(f32, f32)>, // (time, scale_value) - NIF usually has uniform scale
                                 // TODO: Add interpolation types if you plan to support more than linear/step
}

// --- Intermediate representation for a full animation sequence ---
#[derive(Debug, Clone)]
pub struct AnimationSequence {
    pub name: String,
    pub start_time: f32,
    pub stop_time: f32,
    pub bone_curves: Vec<BoneAnimationCurve>,
    // TODO: Add looping information if available from NiTextKeyExtraData
}
#[derive(Resource, Debug, Default)]
pub struct BoneMap {
    pub root_skeleton_entity: Option<Entity>,
    pub entity_map: HashMap<String, Entity>,
}

// --- Assume your AnimationHandles Resource ---
#[derive(Resource, Debug, Default)]
pub struct AnimationHandles {
    pub main_clip: Option<Handle<AnimationClip>>,
}

#[derive(Debug, Default, Clone)]
pub struct EntityPath {
    parts: Vec<Name>,
}
// --- Helper Lerp/Slerp Functions ---
fn quat_slerp(a: &Quat, b: &Quat, t: f32) -> Quat {
    // Use normalize based on E0599 help message
    a.normalize().slerp(b.normalize(), t)
}
pub fn build_animation_clip_system(
    mut commands: Commands,
    bone_map_res: Res<BoneMap>,
    nif_assets: Res<Assets<Nif>>,
    mut animations: ResMut<Assets<AnimationClip>>,
    names_query: Query<&Name>, // Query needed to get Name for EntityPath
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    needs_animator_q: Query<(Entity, &NeedsNifAnimator)>,
    has_parent_q: Query<&Parent>,
    transforms: Query<&Transform>,
    mut x: Local<u128>,
) {
    if bone_map_res.root_skeleton_entity.is_none() {
        return;
    };
    if *x < 50 {
        println!("x: {}", *x);
        *x += 1;
        return;
    }
    for (entity, nif_handle_component) in needs_animator_q.iter() {
        println!("bone map: {:?}", bone_map_res.entity_map);
        for (_, bone) in bone_map_res.entity_map.iter() {
            if let Ok(bone_transform) = transforms.get(*bone) {
                println!("bone: {}, transform: {:?}", bone, bone_transform);
            }
        }
        println!("called for entity {}", entity);
        let nif_handle = &nif_handle_component.0;
        // **Check if the asset for THIS entity is loaded NOW**
        // This uses Assets::get, polling the current state of loaded assets
        let Some(nif) = nif_assets.get(&*nif_handle) else {
            continue;
        };
        main_example(&nif.raw_parsed);
        //       main_example(&nif.raw_parsed);
        let Ok(mut nif_animations_map) = extract_animations_from_base_anim(&nif.raw_parsed) else {
            commands.entity(entity).remove::<NeedsNifAnimator>();
            continue;
        };
        for (name, animation) in &nif_animations_map {
            println!("name: {}", name);
        }

        // info!("Building AnimationClip using SampleCurve and add_curve_to_target...");

        let mut animation_player = AnimationPlayer::default();
        let mut bone_entity: Option<Entity> = None;
        let mut animation_clip = AnimationClip::default();
        let Some(nif_animation) = nif_animations_map.get("swimknockout") else {
            warn!(
                "Animation  not found in extracted map for NIF {:?}.",
                nif_handle.id()
            );
            commands.entity(entity).remove::<NeedsNifAnimator>();
            continue;
        };
        for bone_curve in nif_animation.bone_curves.iter() {
            info!("Bone: {}", bone_curve.target_bone_name);
            if !bone_curve.translations.is_empty() {
                info!("  Trans[0]: {:?}", bone_curve.translations[0]);
            }
            if !bone_curve.rotations.is_empty() {
                info!("  Rots[0]: {:?}", bone_curve.rotations[0]);
            }
            let translation_curves = make_auto_or_constant_curve(
                &bone_curve.translations,
                Interval::new(nif_animation.start_time, nif_animation.stop_time),
            );
            let rotation_curves = make_auto_or_constant_curve(
                &bone_curve.rotations,
                Interval::new(nif_animation.start_time, nif_animation.stop_time),
            );
            for (string, bone) in bone_map_res.entity_map.iter() {
                println!("bone name: {}", bone_curve.target_bone_name);
                if *string == format!("NiNode: {}", bone_curve.target_bone_name) {
                    bone_entity = Some(*bone);
                    break;
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
                player: entity,
            });
        }

        let handle = animations.add(animation_clip);
        let (animation_graph, animation_node_index) = AnimationGraph::from_clip(handle);
        let animation_graph_handle = animation_graphs.add(animation_graph);
        animation_player.play(animation_node_index).repeat();
        commands
            .entity(entity)
            .insert(AnimationGraphHandle(animation_graph_handle));
        println!("adding animation player to entity {}", entity);
        commands.entity(entity).insert(animation_player);
        commands.entity(entity).remove::<NeedsNifAnimator>();
    }
}
pub fn find_bone_path(has_parent_q: &Query<&Parent>, entity: &Entity, mut path: String) -> String {
    path.push_str(&entity.to_string());
    if let Ok(parent) = has_parent_q.get(*entity) {
        find_bone_path(has_parent_q, &parent, path)
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
        for (i, block) in nif_data.blocks.iter().enumerate() {
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
                println!(
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
    let mut current_anim_name: Option<String> = None;
    let mut current_anim_start_time: Option<f32> = None;

    for text_key in text_keys {
        let key_value_lower = text_key.value.to_lowercase();
        if key_value_lower.contains("loop start") || key_value_lower.contains("start") {
            current_anim_name = Some(
                key_value_lower
                    .split(':')
                    .next()
                    .unwrap_or("unnamed_anim")
                    .replace("loop start", "")
                    .replace("start", "")
                    .trim()
                    .to_string(),
            );
            if current_anim_name.as_deref() == Some("") {
                // Handle cases like just "loop start"
                current_anim_name = Some("default_loop".to_string());
            }
            current_anim_start_time = Some(text_key.time);
        } else if (key_value_lower.contains("loop stop") || key_value_lower.contains("stop"))
            && current_anim_name.is_some()
            && current_anim_start_time.is_some()
        {
            let anim_name = current_anim_name.take().unwrap();
            let start_time = current_anim_start_time.take().unwrap();
            let stop_time = text_key.time;

            let mut sequence = AnimationSequence {
                name: anim_name.clone(),
                start_time,
                stop_time,
                bone_curves: Vec::new(),
            };

            // 4. For each defined sequence, gather all relevant bone animations
            for (target_node_idx, controllers) in &all_bone_controllers {
                let target_node_block = nif_data.blocks.get(*target_node_idx).ok_or_else(|| {
                    format!(
                        "Invalid target node index {} for controller",
                        target_node_idx
                    )
                })?;

                let bone_name = match target_node_block {
                    ParsedBlock::Node(node) => node.av_base.net_base.name.clone(),
                    // TODO: Handle if other types can be animation targets, though NiNode is typical
                    _ => format!("UnnamedBone_{}", target_node_idx),
                };

                let mut bone_curve = BoneAnimationCurve {
                    target_bone_name: bone_name,
                    rotations: Vec::new(),
                    translations: Vec::new(),
                    scales: Vec::new(),
                };

                for kfc in controllers {
                    // Check if this controller is active within the current sequence's time range
                    // (KFC start/stop times might be absolute or relative to sequence, often absolute in KF files)
                    // For base_anim.nif, controllers usually span the entire timeline and text keys define segments.
                    // So, we filter keys from KeyframeData based on sequence start/stop.

                    if let Some(kfd_block_idx) = kfc.keyframe_data {
                        if let Some(keyframe_data) =
                            get_block(nif_data, Some(kfd_block_idx), as_keyframe_data)
                        {
                            // Extract rotations
                            for key_quat in &keyframe_data.quaternion_keys {
                                if key_quat.time >= start_time && key_quat.time <= stop_time {
                                    // TODO: Handle different KeyTypes for interpolation if necessary.
                                    // For simple extraction, we just take the value.
                                    // Bevy's animation system will handle interpolation between these.
                                    bone_curve.rotations.push((
                                        key_quat.time - start_time, // Time relative to sequence start
                                        to_bevy_quat(key_quat.value),
                                    ));
                                }
                            }
                            // Extract translations
                            for key_vec3 in &keyframe_data.translations {
                                if key_vec3.time >= start_time && key_vec3.time <= stop_time {
                                    bone_curve.translations.push((
                                        key_vec3.time - start_time,
                                        to_bevy_vec3(key_vec3.value),
                                    ));
                                }
                            }
                            // Extract scales
                            for key_float in &keyframe_data.scales {
                                if key_float.time >= start_time && key_float.time <= stop_time {
                                    bone_curve
                                        .scales
                                        .push((key_float.time - start_time, key_float.value));
                                }
                            }
                        }
                    }
                }

                // Add the bone curve if it has any keyframes for this sequence
                if !bone_curve.rotations.is_empty()
                    || !bone_curve.translations.is_empty()
                    || !bone_curve.scales.is_empty()
                {
                    // Sort keyframes by time, just in case they aren't already
                    bone_curve
                        .rotations
                        .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                    bone_curve
                        .translations
                        .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                    bone_curve
                        .scales
                        .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                    sequence.bone_curves.push(bone_curve);
                }
            }
            if !sequence.bone_curves.is_empty() {
                animation_sequences.insert(sequence.name.clone(), sequence);
            }
        }
    }

    if animation_sequences.is_empty() && !text_keys.is_empty() {
        // Handle case where there might be no explicit "start"/"stop" but text keys imply one full sequence
        // e.g. older KF files or simple animations.
        // This is a heuristic.
        let mut start_time = text_keys.first().map_or(0.0, |tk| tk.time);
        let mut stop_time = text_keys.last().map_or(0.0, |tk| tk.time);
        if stop_time <= start_time && !nif_data.blocks.is_empty() {
            // try to find a max time from controllers
            let mut max_controller_time = 0.0f32;
            for (_, controllers) in &all_bone_controllers {
                for kfc in controllers {
                    max_controller_time = max_controller_time.max(kfc.stop_time);
                    if let Some(kfd_block_idx) = kfc.keyframe_data {
                        if let Some(keyframe_data) =
                            get_block(nif_data, Some(kfd_block_idx), as_keyframe_data)
                        {
                            if let Some(last_rot) = keyframe_data.quaternion_keys.last() {
                                max_controller_time = max_controller_time.max(last_rot.time);
                            }
                            if let Some(last_trans) = keyframe_data.translations.last() {
                                max_controller_time = max_controller_time.max(last_trans.time);
                            }
                            if let Some(last_scale) = keyframe_data.scales.last() {
                                max_controller_time = max_controller_time.max(last_scale.time);
                            }
                        }
                    }
                }
            }
            if max_controller_time > stop_time {
                stop_time = max_controller_time;
            }
        }

        if stop_time > start_time {
            println!(
                "No explicit start/stop in text keys, creating one sequence from {:.2} to {:.2}",
                start_time, stop_time
            );
            let mut sequence = AnimationSequence {
                name: "default_animation".to_string(), // Or derive from file name
                start_time,
                stop_time,
                bone_curves: Vec::new(),
            };
            for (target_node_idx, controllers) in &all_bone_controllers {
                let target_node_block = nif_data.blocks.get(*target_node_idx).ok_or_else(|| {
                    format!(
                        "Invalid target node index {} for controller",
                        target_node_idx
                    )
                })?;
                let bone_name = match target_node_block {
                    ParsedBlock::Node(node) => node.av_base.net_base.name.clone(),
                    _ => format!("UnnamedBone_{}", target_node_idx),
                };
                let mut bone_curve = BoneAnimationCurve {
                    target_bone_name: bone_name,
                    rotations: Vec::new(),
                    translations: Vec::new(),
                    scales: Vec::new(),
                };
                for kfc in controllers {
                    if let Some(kfd_block_idx) = kfc.keyframe_data {
                        if let Some(keyframe_data) =
                            get_block(nif_data, Some(kfd_block_idx), as_keyframe_data)
                        {
                            for key_quat in &keyframe_data.quaternion_keys {
                                if key_quat.time >= start_time && key_quat.time <= stop_time {
                                    bone_curve.rotations.push((
                                        key_quat.time - start_time,
                                        to_bevy_quat(key_quat.value),
                                    ));
                                }
                            }
                            for key_vec3 in &keyframe_data.translations {
                                if key_vec3.time >= start_time && key_vec3.time <= stop_time {
                                    bone_curve.translations.push((
                                        key_vec3.time - start_time,
                                        to_bevy_vec3(key_vec3.value),
                                    ));
                                }
                            }
                            for key_float in &keyframe_data.scales {
                                if key_float.time >= start_time && key_float.time <= stop_time {
                                    bone_curve
                                        .scales
                                        .push((key_float.time - start_time, key_float.value));
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
                    sequence.bone_curves.push(bone_curve);
                }
            }
            if !sequence.bone_curves.is_empty() {
                //animation_sequences.push(sequence);
            }
        }
    }

    Ok(animation_sequences)
}

// Example usage (you'll need to load `ParsedNifData` first)
pub fn main_example(parsed_nif_data: &ParsedNifData) {
    match extract_animations_from_base_anim(parsed_nif_data) {
        Ok(animations) => {
            println!(
                "Successfully extracted {} animation sequences:",
                animations.len()
            );
            let mut x = 0;
            for (name, anim_seq) in animations {
                x += 1;
                println!(
                    "  Sequence: {}, Start: {:.2}, Stop: {:.2}, Bones Animated: {}",
                    anim_seq.name,
                    anim_seq.start_time,
                    anim_seq.stop_time,
                    anim_seq.bone_curves.len()
                );
                for bone_curve in anim_seq.bone_curves.iter() {
                    // Print details for first bone only
                    println!("    Bone: {}", bone_curve.target_bone_name);
                    if !bone_curve.translations.is_empty() {
                        for key in &bone_curve.translations {
                            println!("anim:{}      Translations: {:?}", x, key);
                        }
                    }
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
fn lerp_vec3(a: &Vec3, b: &Vec3, t: f32) -> Vec3 {
    a.lerp(*b, t)
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
