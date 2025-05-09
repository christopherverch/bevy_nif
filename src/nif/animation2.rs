// --- Necessary Imports ---

// --- Bevy Imports (Corrected for Bevy 0.16+) ---
use bevy::animation::{AnimationClip, AnimationCurve, CurveId, SampleCurve}; // Import SampleCurve, CurveId. Keep AnimationClip. Removed path::CurvePath.
use bevy::asset::{Assets, Handle}; // Keep Assets, Handle
use bevy::ecs::entity::Entity;
use bevy::hierarchy::path::EntityPath; // *** CORRECTED IMPORT PATH ***
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
use bevy::reflect::path::PropertyPath; // *** CORRECTED IMPORT PATH ***
use bevy::transform::components::Transform;
use std::collections::HashMap;
use std::sync::Arc;

use crate::{ParsedBlock, ParsedNifData}; // Needed for add_curve_boxed potentially, or boxing

// --- Assume your Key structs, KeyType, NiKeyframeData, NiKeyframeController ---
// --- ParsedNifData, RecordLink, Vector3 etc. are defined exactly as you provided ---

#[derive(Resource, Debug, Default)]
pub struct BoneMap(pub HashMap<String, Entity>);

// --- Assume your AnimationHandles Resource ---
#[derive(Resource, Debug, Default)]
pub struct AnimationHandles {
    pub main_clip: Option<Handle<AnimationClip>>,
}

// --- Helper Lerp/Slerp Functions ---
fn vec3_lerp(a: &Vec3, b: &Vec3, t: f32) -> Vec3 {
    a.lerp(*b, t)
}
fn quat_slerp(a: &Quat, b: &Quat, t: f32) -> Quat {
    // Use normalize based on E0599 help message
    a.normalize().slerp(b.normalize(), t)
}

// --- System to Build the Animation Clip (Corrected for Bevy 0.16+) ---

pub fn build_animation_clip_system(
    mut commands: Commands,
    parsed_nif_data_res: Option<Res<ParsedNifData>>,
    bone_map_res: Option<Res<BoneMap>>,
    mut animations: ResMut<Assets<AnimationClip>>,
    names_query: Query<&Name>, // Query needed to get Name for EntityPath
) {
    let Some(parsed_nif) = parsed_nif_data_res else {
        return;
    };
    let Some(bone_map) = bone_map_res else {
        return;
    };

    // info!("Building AnimationClip using SampleCurve and add_curve_to_target...");

    let mut clip = AnimationClip::default();
    let mut curves_added = 0;

    for block in &parsed_nif.blocks {
        if let ParsedBlock::KeyframeController(controller) = block {
            let Some(data_index) = controller.keyframe_data else {
                continue;
            };
            let Some(target_index) = controller.target else {
                continue;
            };
            let keyframe_data = match parsed_nif.blocks.get(data_index) {
                Some(ParsedBlock::KeyframeData(data)) => data,
                _ => continue,
            };
            let target_node_name_str = match parsed_nif.blocks.get(target_index) {
                Some(ParsedBlock::Node(node)) => &node.av_base.net_base.name,
                _ => continue,
            };
            let Some(target_entity) = bone_map.0.get(target_node_name_str).copied() else {
                continue;
            };
            let Ok(target_entity_name) = names_query.get(target_entity) else {
                continue;
            };

            // --- Construct Paths (Assuming EntityPath/PropertyPath are in prelude) ---
            let entity_path = EntityPath {
                parts: vec![target_entity_name.clone()],
            };

            // --- Create and Add Curves using add_curve_to_target ---

            // Rotation Curve
            if !keyframe_data.quaternion_keys.is_empty() {
                let timestamps: Vec<f32> = keyframe_data
                    .quaternion_keys
                    .iter()
                    .map(|k| k.time)
                    .collect();
                let values: Vec<Quat> = keyframe_data
                    .quaternion_keys
                    .iter()
                    .map(|k| k.value)
                    .collect();
                let curve = SampleCurve::new(timestamps, values, quat_slerp); // Use SampleCurve
                let property_path = PropertyPath::from_dot_string("Transform.rotation");

                // Construct AnimationTargetId (Assuming prelude)
                let target_id = AnimationTargetId::EntityPath {
                    path: entity_path.clone(),
                    property: property_path,
                };

                // *** Use add_curve_to_target based on E0599 compiler help ***
                clip.add_curve_to_target(target_id, curve);
                curves_added += 1;
            }

            // Translation Curve
            if !keyframe_data.translations.is_empty() {
                let timestamps: Vec<f32> =
                    keyframe_data.translations.iter().map(|k| k.time).collect();
                let values: Vec<Vec3> = keyframe_data
                    .translations
                    .iter()
                    .map(|k| Vec3::from(k.value.0))
                    .collect();
                let curve = SampleCurve::new(timestamps, values, vec3_lerp);
                let property_path = PropertyPath::from_dot_string("Transform.translation");
                let target_id = AnimationTargetId::EntityPath {
                    path: entity_path.clone(),
                    property: property_path,
                };
                clip.add_curve_to_target(target_id, curve);
                curves_added += 1;
            }

            // Scale Curve
            if !keyframe_data.scales.is_empty() {
                let timestamps: Vec<f32> = keyframe_data.scales.iter().map(|k| k.time).collect();
                let values: Vec<Vec3> = keyframe_data
                    .scales
                    .iter()
                    .map(|k| Vec3::splat(k.value))
                    .collect();
                let curve = SampleCurve::new(timestamps, values, vec3_lerp);
                let property_path = PropertyPath::from_dot_string("Transform.scale");
                let target_id = AnimationTargetId::EntityPath {
                    path: entity_path.clone(),
                    property: property_path,
                };
                clip.add_curve_to_target(target_id, curve);
                curves_added += 1;
            }
        }
    }

    if curves_added > 0 {
        // info!("AnimationClip created with {} curves.", curves_added);
        let handle = animations.add(clip);
        commands.insert_resource(AnimationHandles {
            main_clip: Some(handle),
        });
    } else {
        // warn!("No animation curves were generated.");
        commands.insert_resource(AnimationHandles::default());
    }
}
