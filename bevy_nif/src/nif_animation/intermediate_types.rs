// src/nif_animation/intermediate_types.rs

use bevy::prelude::{Quat, Vec3}; // Using Bevy's math types as in your animation.rs_new.txt

// --- Intermediate representation for an animation curve for a specific bone ---
// This structure holds the keyframes for a single bone within an AnimationSequence.
// Times are relative to the start of the parent AnimationSequence.
#[derive(Default, Debug, Clone)]
pub struct BoneAnimationCurve {
    pub target_bone_name: String, // Name of the NiNode (bone) this curve applies to
    pub rotations: Vec<(f32, Quat)>, // (time, rotation_value)
    pub translations: Vec<(f32, Vec3)>, // (time, translation_value)
    pub scales: Vec<(f32, Vec3)>, // (time, scale_value) - NIF often has uniform scale
                                  // Consider adding fields here if you need to store the original NIF interpolation type
                                  // for each track (e.g., Linear, Quadratic, Constant) if Bevy's AnimationClip
                                  // needs this hint or if you perform sampling/conversion later.
                                  // pub rotation_interpolation: NifKeyType, // (NifKeyType would be from your parser's types)
                                  // pub translation_interpolation: NifKeyType,
                                  // pub scale_interpolation: NifKeyType,
}

// --- Represents a text-based event within an animation sequence ---
#[derive(Debug, Clone)]
pub struct TextKeyEvent {
    pub time: f32,     // Time relative to the start of its parent AnimationSequence
    pub value: String, // The full, original text key string (e.g., "SoundGen:LFoot", "HandToHand:Chop Min Attack")
}

// --- Intermediate representation of a complete NIF animation sequence ---
// This is populated by the text key parsing logic.
#[derive(Debug, Clone)]
pub struct AnimationSequence {
    pub name: String, // The full, original-cased animation group name (e.g., "HandToHand:Chop", "Idle")
    pub abs_start_time: f32, // Absolute start time from the NIF's text key timeline (for reference/debugging)
    pub abs_stop_time: f32, // Absolute stop time from the NIF's text key timeline (for reference/debugging)

    pub bone_curves: Vec<BoneAnimationCurve>, // All bone animation data for this sequence
    pub events: Vec<TextKeyEvent>, // All text-based events occurring within this sequence

    pub loop_start_time: Option<f32>, // Loop start time, relative to this sequence's own start (0.0)
    pub loop_stop_time: Option<f32>,  // Loop stop time, relative to this sequence's own start (0.0)

    // Storing the initial position of Bip01 (or the root motion bone) at the
    // start of this sequence can be useful for certain root motion calculations.
    pub initial_position: Vec3,
    // This flag is typically set by your `split_animation_for_looping` function
    // after this AnimationSequence is initially extracted.
    pub is_startup_to_loop: bool,
}
