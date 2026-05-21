use std::collections::BTreeMap;

use bevy::ecs::component::Component;
use esm_parser_lib::RefId;

// 1. Sub-struct (Not a component)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectKey {
    pub id: RefId,
    pub arg: Option<RefId>,
}

// 2. Sub-struct (Not a component)
#[derive(Debug, Clone, Copy, Default)]
pub struct EffectParam {
    pub base: i32,
    pub modifier: f32,
}

impl EffectParam {
    pub fn get_magnitude(&self) -> f32 {
        self.base as f32 + self.modifier
    }
}

// 3. THE COMPONENT
#[derive(Component, Debug, Clone, Default)]
pub struct MagicEffects {
    /// Maps the EffectKey to its current Live values
    pub collection: BTreeMap<EffectKey, EffectParam>,
}

impl MagicEffects {
    // Replicate OpenMW helper methods here
    pub fn get_or_default(&self, id: &RefId) -> EffectParam {
        // Find first effect matching ID if arg is ignored
        self.collection
            .iter()
            .find(|(k, _)| &k.id == id)
            .map(|(_, p)| *p)
            .unwrap_or_default()
    }
}
