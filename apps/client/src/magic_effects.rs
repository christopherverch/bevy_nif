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
    pub fn get(&self, key: &EffectKey) -> Option<EffectParam> {
        self.collection.get(key).copied()
    }

    /// Matches: EffectParam MagicEffects::getOrDefault(const EffectKey& key)
    pub fn get_or_default_by_key(&self, key: &EffectKey) -> EffectParam {
        self.get(key).unwrap_or_default()
    }

    /// Matches: EffectParam MagicEffects::getOrDefault(ESM::RefId effectId)
    pub fn get_or_default(&self, effect_id: RefId) -> EffectParam {
        // Constructs the key and delegates
        let key = EffectKey {
            id: effect_id,
            arg: None,
        };
        self.get_or_default_by_key(&key)
    }
}
