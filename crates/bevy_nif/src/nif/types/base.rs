#![allow(dead_code)]
use bevy::prelude::*;
use bevy::reflect::TypePath;
use std::fmt::Debug;

use crate::nif::error::ParseError;

/// Represents links to other records (usually by index in the main Vec<NifRecord>)
pub type RecordLink = Option<usize>;

#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct NifHeader {
    pub version_string: String,
    pub file_version: u32, // Represents the uint version read (e.g., 0x04000002)
    pub num_blocks: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Matrix3x3(pub [[f32; 3]; 3]); // Wrap the 2D array

impl Default for Matrix3x3 {
    fn default() -> Self {
        Matrix3x3([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Matrix4x4(pub [[f32; 4]; 4]); // Row-major or column-major? Assume standard [row][col] for now

#[derive(Debug, Clone, Copy, Default)]
pub struct Plane {
    pub normal: Vec3,
    pub constant: f32,
}

/// Represents the C++ NiTransform struct
#[derive(Debug, Clone, Default)]
pub struct NiTransform {
    pub rotation: Matrix3x3,
    pub translation: Vec3,
    pub scale: f32,
}

#[derive(Debug, Clone, Default)]
pub struct BoundingSphere {
    pub center: Vec3,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    // NIF format often stores Box as Center, Axes (Rotation Matrix), Extent (Half-dimensions)
    pub center: Vec3,
    pub axes: Matrix3x3,
    pub extent: Vec3, // Extents along each axis (half-sizes)
}
#[derive(Debug, Clone, Copy)]
pub struct BoundingCapsule {
    pub center: Vec3,
    pub axis: Vec3,
    pub extent: f32,
    pub radius: f32,
}
#[derive(Debug, Clone)]
pub enum BoundingVolume {
    Sphere(BoundingSphere),
    Box(BoundingBox),
    Capsule(BoundingCapsule),
    Union(Vec<BoundingVolume>),
}
#[repr(u32)] // Ensures the enum has the same representation as a u32
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundingVolumeType {
    Sphere = 0,
    Box = 1,
    Capsule = 2,
    Union = 4,
    Lozenge = 3,
    HalfSpace = 5,
}
impl TryFrom<u32> for BoundingVolumeType {
    type Error = ParseError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Sphere),
            1 => Ok(Self::Box),
            2 => Ok(Self::Capsule),
            3 => Ok(Self::Lozenge),
            4 => Ok(Self::Union),
            5 => Ok(Self::HalfSpace),
            _ => Err(ParseError::InvalidData(format!(
                "Unknown BoundingVolumeType value: {}",
                value
            ))),
        }
    }
}
