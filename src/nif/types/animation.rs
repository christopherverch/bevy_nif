#![allow(dead_code)]
use super::base::{RecordLink, Vector3};
use super::{NiObjectNET, NiTransform};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use std::fmt::Debug;
use std::ops::Deref;
// Using Bevy's Quat directly if possible, otherwise define alias
pub type Quaternion = bevy::math::Quat; // Assuming you use Bevy's Quat elsewhere
#[derive(Asset, Clone, Debug, Default, TypePath)] // Add Asset, TypePath if this is a top-level asset
pub struct NiKeyframeController {
    // Corresponds to NiTimeController base fields for this version
    pub next_controller: RecordLink, // Link to next NiTimeController in the chain
    pub flags: u16,                  // Animation flags (active, looping mode, etc.)
    pub frequency: f32,
    pub phase: f32,
    pub start_time: f32,
    pub stop_time: f32,
    pub target: RecordLink, // Link to the controlled object (usually NiAVObject/NiNode)
    // Field specific to NiKeyframeController in v4.0.0.2
    pub keyframe_data: RecordLink, // Link to NiKeyframeData holding the actual keys
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyType {
    // Interpolation type for keys
    #[default]
    Linear,
    Quadratic,   // Bezier interpolation
    TBC,         // Tension Bias Continuity interpolation
    XyzRotation, // Euler angles (Deprecated)
    Const,       // Step function - value is constant between keys
    Unknown(u32),
}

impl From<u32> for KeyType {
    fn from(value: u32) -> Self {
        match value {
            1 => KeyType::Linear,
            2 => KeyType::Quadratic,
            3 => KeyType::TBC,
            4 => KeyType::XyzRotation,
            5 => KeyType::Const,
            _ => KeyType::Unknown(value),
        }
    }
}

#[derive(Asset, TypePath, Debug, Clone, Copy, Default)]
pub struct KeyFloat {
    pub time: f32,
    pub value: f32,
    // --- ADDED/MODIFIED: Store extra data ---
    pub forward_tangent: Option<f32>,  // For Quadratic keys
    pub backward_tangent: Option<f32>, // For Quadratic keys
    pub tension: Option<f32>,          // For TBC keys
    pub bias: Option<f32>,             // For TBC keys
    pub continuity: Option<f32>,       // For TBC keys
}

#[derive(Asset, TypePath, Debug, Clone, Copy, Default)]
pub struct KeyVec3 {
    pub time: f32,
    pub value: Vector3,
    // --- ADDED/MODIFIED: Store extra data ---
    pub forward_tangent: Option<Vector3>,  // For Quadratic keys
    pub backward_tangent: Option<Vector3>, // For Quadratic keys
    pub tension: Option<f32>,              // For TBC keys
    pub bias: Option<f32>,                 // For TBC keys
    pub continuity: Option<f32>,           // For TBC keys
}

#[derive(Asset, TypePath, Debug, Clone, Copy, Default)]
pub struct KeyQuaternion {
    pub time: f32,
    pub value: Quaternion,
    // --- ADDED/MODIFIED: Store extra data ---
    // Assuming tangents are also Quaternions for Quadratic keys
    pub forward_tangent: Option<Quaternion>,
    pub backward_tangent: Option<Quaternion>,
    // TBC parameters
    pub tension: Option<f32>,
    pub bias: Option<f32>,
    pub continuity: Option<f32>,
}

#[derive(Asset, TypePath, Clone, Debug, Default)] // Added Asset, TypePath
pub struct NiKeyframeData {
    pub rotation_type: Option<KeyType>, // Rotation type (if keys exist)
    pub quaternion_keys: Vec<KeyQuaternion>,
    pub translation_interp: KeyType, // Interpolation type for ALL translation keys
    pub translations: Vec<KeyVec3>,
    pub scale_interp: KeyType, // Interpolation type for ALL scale keys
    pub scales: Vec<KeyFloat>,
}

// Represents a single text keyframe
#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct TextKey {
    pub time: f32,
    pub value: String,
}

#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct NiGeomMorpherController {
    // Fields from NiTimeController base for NIF v4.0.0.2
    pub next_controller: RecordLink,
    pub flags: u16,
    pub frequency: f32,
    pub phase: f32,
    pub start_time: f32,
    pub stop_time: f32,
    pub target: RecordLink, // Link to the NiNode/NiTriShape being morphed
    // Field specific to NiGeomMorpherController in NIF v4.0.0.2
    pub morph_data: RecordLink, // Link to NiMorphData
    pub always_update: bool,    // Read as byte, store as bool
}
#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct MorphTarget {
    // Added fields based on NifSkope view for NiMorphData structure
    pub num_keys: u32, // Number of interpolation keys for this target's weight
    pub interpolation: KeyType, // Interpolation type for the keys
    pub keys: Vec<KeyFloat>, // Interpolation keys (time/value pairs, maybe more for non-linear)
    // Vertex data remains
    pub vertices: Vec<Vector3>, // The actual vertex data for this target
}

// --- NiMorphData struct stays the same ---
#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct NiMorphData {
    pub num_morph_targets: u32,
    pub num_vertices: u32,
    pub relative_targets: bool,
    pub morph_targets: Vec<MorphTarget>, // Holds the struct above
}
#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct NiSkinInstance {
    // Note: Inherits NiObject, not NiObjectNET
    pub data: RecordLink,          // Link to NiSkinData block (Required)
    pub skeleton_root: RecordLink, // Link to the root NiNode of the skeleton (Required)
    pub num_bones: u32,            // Number of bones influencing the mesh
    pub bones: Vec<RecordLink>,    // List of links to the NiNode bones (size = num_bones)
                                   // NiSkinPartition link is absent in v4.0.0.2
}
#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct BoneVertData {
    pub index: u16,  // Index into the original mesh's vertex list
    pub weight: f32, // Bone weight (influence) for this vertex (0.0 to 1.0)
}
#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct BoneData {
    // Fields now match NifSkope screenshot order and types
    pub bone_transform: NiTransform, // Contains Rot(Matrix33), Trans(Vector3), Scale(float)
    pub bounding_sphere_offset: Vector3,
    pub bounding_sphere_radius: f32,
    pub num_vertices: u16, // ushort in NifSkope
    pub vertex_weights: Vec<BoneVertData>,
    // REMOVED unknown_16_bytes field
}

/// Contains the skinning data for a mesh, including bone transforms and vertex weights.
/// Referenced by NiSkinInstance.
#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct NiSkinData {
    // Note: Inherits NiObject (no fields read from base here)
    /// Overall transformation applied to the skin *before* bone influences.
    pub skin_transform: NiTransform,
    /// Number of bones influencing the mesh. Should generally match
    /// the count in the referencing NiSkinInstance.
    pub num_bones: u32,
    /// List containing transform and weighting data for each bone.
    pub bone_list: Vec<BoneData>,
}
#[derive(Debug, Clone)]
pub struct NiStreamHeader {
    pub layout: u32,
    pub num_objects: u32,
    pub object_types: Vec<String>, // Assuming NiStringPalette is Vec<String>
    pub object_sizes: Vec<u32>,
}

// Structure for the Footer block within NiSequenceStreamHelper
#[derive(Debug, Clone)]
pub struct NiStreamFooter {
    pub num_objects: u32,
}

#[derive(Debug, Clone, Default, TypePath)] // Add TypePath if needed
pub struct NiSequenceStreamHelper {
    pub net_base: NiObjectNET,
    // No additional fields specific to NiSequenceStreamHelper itself
}

// Optional Deref if you want to access NiObjectNET fields directly
impl Deref for NiSequenceStreamHelper {
    type Target = NiObjectNET;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.net_base
    }
}
