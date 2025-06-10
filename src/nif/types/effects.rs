#![allow(dead_code)]
use super::base::{Matrix3x3, Plane, RecordLink};
use super::scene::NiAVObject;
use super::textures::{ClampMode, FilterMode};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use std::fmt::Debug;
use std::ops::Deref;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EffectType {
    #[default]
    ProjectedLight,
    ProjectedShadow,
    EnvironmentMap,
    FogMap,
    Unknown(u32),
}
impl From<u32> for EffectType {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::ProjectedLight,
            1 => Self::ProjectedShadow,
            2 => Self::EnvironmentMap,
            3 => Self::FogMap,
            _ => Self::Unknown(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoordGenType {
    #[default]
    WorldParallel, // TexCoordGen::WORLD_PARALLEL
    WorldPerspective, // TexCoordGen::WORLD_PERSPECTIVE
    SphereMap,        // TexCoordGen::SPHERE_MAP
    SpecularCubeMap,  // TexCoordGen::SPECULAR_CUBE_MAP
    DiffuseCubeMap,   // TexCoordGen::DIFFUSE_CUBE_MAP
    Unknown(u32),
}
impl From<u32> for CoordGenType {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::WorldParallel,
            1 => Self::WorldPerspective,
            2 => Self::SphereMap,
            3 => Self::SpecularCubeMap,
            4 => Self::DiffuseCubeMap,
            _ => Self::Unknown(value),
        }
    }
}

// ClippingPlane enum isn't strictly needed if we just store the bool + Plane struct
#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct NiDynamicEffect {
    pub av_base: NiAVObject,     // Composition of NiAVObject
    pub num_affected_nodes: u32, // Count read from file
    pub affected_nodes: Vec<RecordLink>, // Links to NiNode(s) affected
                                 // Switch State is not present in v4.0.0.2
}

// --- MODIFIED: NiTextureEffect Struct ---
#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct NiTextureEffect {
    // *** CHANGED: Contains NiDynamicEffect now ***
    pub dynamic_effect_base: NiDynamicEffect,
    // *** Specific fields remain the same ***
    pub model_projection_matrix: Matrix3x3,
    pub model_projection_translation: Vec3,
    pub texture_filtering: FilterMode,
    pub texture_clamping: ClampMode,
    pub texture_type: EffectType,
    pub coordinate_generation_type: CoordGenType,
    pub source_texture: RecordLink,
    pub enable_plane: bool,
    pub plane: Option<Plane>,
    pub ps2_l: i16,
    pub ps2_k: i16,
    pub unknown_short: u16,
}

// --- Deref Implementations ---
// Add Deref for NiDynamicEffect -> NiAVObject
impl Deref for NiDynamicEffect {
    type Target = NiAVObject;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.av_base
    }
}

// Modify Deref for NiTextureEffect -> NiDynamicEffect
impl Deref for NiTextureEffect {
    type Target = NiDynamicEffect; // <<< Changed target
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.dynamic_effect_base // <<< Changed field
    }
}
