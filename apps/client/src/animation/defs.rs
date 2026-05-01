use bevy_nif::nif_animation::BlendMask;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AnimationConfig {
    pub name: String,
    pub repeat: RepeatAnim,
    pub priority: u8,
    pub mask: BlendMask,
}
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RepeatAnim {
    Never,
    Forever,
    Count(u32),
}
