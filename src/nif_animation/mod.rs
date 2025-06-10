// src/nif_animation/mod.rs

// Declare your modules
pub mod animation_setup_system;
pub mod bevy_types; // Contains NifAnimator, AnimationDefinition, BlendMask, NifEventType, etc.
pub mod intermediate_types; // Contains AnimationSequence, BoneAnimationCurve, TextKeyEvent
pub mod parser_helpers; // Contains parse_nif_text_key_value, KNOWN_GENERIC_EVENT_GROUP_NAMES, etc.
pub mod text_key_parser; // Contains extract_animation_sequences_from_text_keys
// Re-export the primary function and key public types for easier use by other parts of your crate
pub use bevy_types::{
    ActiveAnimNif, AnimationDefinition, AnimationRepeatBehavior, AnimationTransitionState,
    BlendMask, NUM_DISCRETE_REGIONS, NifAnimator, NifAnimatorAdded, NifEvent, NifEventType,
    REGION_INDEX_LEFT_ARM, REGION_INDEX_LOWER_BODY, REGION_INDEX_RIGHT_ARM, REGION_INDEX_TORSO,
    REGION_ROOT_LEFT_ARM, REGION_ROOT_LOWER_BODY, REGION_ROOT_RIGHT_ARM, REGION_ROOT_TORSO,
    SkeletonMap,
};
pub use intermediate_types::{AnimationSequence, BoneAnimationCurve, TextKeyEvent};

// Any other functions from your original animation.rs that are public and still needed,
// like build_animation_clip_system (which would now use the new extract function),
// split_animation_for_looping, process_nif_animation, determine_bone_primary_region_index etc.
// would either be moved into one of these modules or into a new `bevy_setup.rs` module.
// For example, `determine_bone_primary_region_index` is now in `parser_helpers.rs`.
// `split_animation_for_looping` and `process_nif_animation` might go into a
// `bevy_clip_converter.rs` or similar.
