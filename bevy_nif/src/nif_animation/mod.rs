pub mod animation_setup_system;
pub mod bevy_types;
pub mod parser_helpers;
pub use bevy_types::{
    AnimationDefinition, AnimationRepeatBehavior, AnimationTransitionState, BlendMask,
    NUM_DISCRETE_REGIONS, NifAnimator, NifAnimatorAdded, NifEvent, NifEventType,
    REGION_INDEX_LEFT_ARM, REGION_INDEX_LOWER_BODY, REGION_INDEX_RIGHT_ARM, REGION_INDEX_TORSO,
    REGION_ROOT_LEFT_ARM, REGION_ROOT_LOWER_BODY, REGION_ROOT_RIGHT_ARM, REGION_ROOT_TORSO,
    SkeletonMap,
};
