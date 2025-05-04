#![allow(dead_code)]
use super::base::{BoundingVolume, NiTransform, RecordLink, Vector3};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use std::fmt::Debug;
use std::ops::Deref;

// --- Structs using Pure Composition ---

#[derive(Debug, Clone, Default)]
pub struct NiObjectNET {
    pub name: String,
    pub extra_data_link: RecordLink,
    pub controller_link: RecordLink,
}

// Example inherent method for NiObjectNET
impl NiObjectNET {
    pub fn name(&self) -> &str {
        &self.name
    }
    // Could add methods for links, etc.
}

#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct NiAVObject {
    pub net_base: NiObjectNET,
    pub flags: u16,
    pub transform: NiTransform,
    pub velocity: Vector3,
    pub properties: Vec<RecordLink>,
    // Add this field to store the bounding sphere when present:
    pub bounding_volume: Option<BoundingVolume>,
}

// Example inherent method for NiAVObject
impl NiAVObject {
    pub fn flags(&self) -> u16 {
        self.flags
    }
    pub fn transform(&self) -> &NiTransform {
        &self.transform
    }
    // etc.
}

#[derive(Debug, Clone, Default)]
pub struct NiNode {
    pub av_base: NiAVObject,
    pub children: Vec<RecordLink>,
    pub effects: Vec<RecordLink>,
}

// Example inherent method for NiNode
impl NiNode {
    pub fn children(&self) -> &[RecordLink] {
        &self.children
    }
    pub fn effects(&self) -> &[RecordLink] {
        &self.effects
    }
}

#[derive(Debug, Clone, Default)]
pub struct NiTriShape {
    pub av_base: NiAVObject,
    pub data_link: RecordLink,
    pub skin_link: RecordLink,
}

// --- Deref Implementations for Automatic Method/Field Forwarding ---

impl Deref for NiAVObject {
    type Target = NiObjectNET; // Target the "parent" struct type
    #[inline]
    fn deref(&self) -> &Self::Target {
        // Return a reference to the composed "parent" instance
        &self.net_base
    }
}
// Note: You would add `impl DerefMut` similarly if mutable access via deref is needed

impl Deref for NiNode {
    type Target = NiAVObject; // Target the "parent" struct type
    #[inline]
    fn deref(&self) -> &Self::Target {
        // Return a reference to the composed "parent" instance
        &self.av_base
    }
}

impl Deref for NiTriShape {
    type Target = NiAVObject;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.av_base
    }
}
