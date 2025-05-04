#![allow(dead_code)]
use super::base::Vector3;
use super::scene::NiObjectNET;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use std::fmt::Debug;
use std::ops::Deref;

#[derive(Debug, Clone, Default)]
pub struct NiProperty {
    pub net_base: NiObjectNET,
}

#[derive(Debug, Clone, Default)]
pub struct NiMaterialProperty {
    pub property_base: NiProperty,
    pub flags: u16, // Present in v4.0.0.2
    pub ambient_color: Vector3,
    pub diffuse_color: Vector3,
    pub specular_color: Vector3,
    pub emissive_color: Vector3,
    pub glossiness: f32,
    pub alpha: f32,
    // emissive_mult is not present in v4.0.0.2
}

// Specific property type: NiAlphaProperty
#[derive(Debug, Clone, Default)]
pub struct NiAlphaProperty {
    pub property_base: NiProperty, // Composition
    pub flags: u16,                // Flags read by NiAlphaProperty::read
    pub threshold: u8,             // Threshold read by NiAlphaProperty::read
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VertexMode {
    #[default]
    SrcIgnore, // Default based on value 0
    SrcEmissive,
    SrcAmbDiff, // Corresponds to Flag Bit 0
    Unknown(u32),
}
impl From<u32> for VertexMode {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::SrcIgnore,
            1 => Self::SrcEmissive,
            2 => Self::SrcAmbDiff,
            _ => Self::Unknown(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LightMode {
    #[default]
    Emissive, // Default based on value 0
    EmissiveAmbientDiffuse, // Corresponds to Flag Bit 1
    Unknown(u32),
}
impl From<u32> for LightMode {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Emissive,
            1 => Self::EmissiveAmbientDiffuse,
            _ => Self::Unknown(value),
        }
    }
}

// --- Add this Struct ---
#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct NiVertexColorProperty {
    pub property_base: NiProperty,        // Composition
    pub flags: u16,                       // Flags determine lighting mode and vertex color source
    pub vertex_mode: Option<VertexMode>,  // Conditional read based on Flags Bit 0
    pub lighting_mode: Option<LightMode>, // Conditional read based on Flags Bit 1
}

// --- Deref Implementations ---
impl Deref for NiProperty {
    // Deref NiProperty -> NiObjectNET
    type Target = NiObjectNET;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.net_base
    }
}

impl Deref for NiMaterialProperty {
    type Target = NiProperty;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.property_base
    }
}

impl Deref for NiAlphaProperty {
    // Deref NiAlphaProperty -> NiProperty
    type Target = NiProperty;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.property_base
    }
}

impl Deref for NiVertexColorProperty {
    type Target = NiProperty;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.property_base
    }
}
