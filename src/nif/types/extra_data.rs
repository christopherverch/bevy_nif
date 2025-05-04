#![allow(dead_code)]
use super::animation::TextKey;
use super::base::RecordLink;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use std::fmt::Debug;
use std::ops::Deref;

// Represents the base NiExtraData object (which just has a name in this version)
#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct NiExtraData {
    pub name: String,
    pub next_extra_data: RecordLink, // *** ADDED this field ***
                                     // Unknown Int 1 is NOT read for v4.0.0.2
}

// NiTextKeyExtraData struct (No changes needed here, uses corrected base)
#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct NiTextKeyExtraData {
    pub base: NiExtraData, // Composition
    pub num_keys: u32,
    pub keys: Vec<TextKey>, // Assumes TextKey struct is defined correctly
}

// Keep Deref for NiTextKeyExtraData -> NiExtraData
impl Deref for NiTextKeyExtraData {
    type Target = NiExtraData;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
