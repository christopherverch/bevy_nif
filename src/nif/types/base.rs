#![allow(dead_code)]
use bevy::prelude::*;
use bevy::reflect::TypePath;
use std::fmt::Debug;

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

#[derive(Debug, Clone, Copy)] // Use Copy if Vec3/Matrix3x3/f32 are Copy
pub struct BoundingBox {
    // NIF format often stores Box as Center, Axes (Rotation Matrix), Extent (Half-dimensions)
    pub center: Vec3,
    pub axes: Matrix3x3, // Or [Vec3; 3] depending on definition
    pub extent: Vec3,    // Extents along each axis (half-sizes)
}
#[derive(Debug, Clone)]
pub enum BoundingVolume {
    Sphere(BoundingSphere),
    Box(BoundingBox),
    // Capsule(BoundingCapsule),
    // Union(Vec<BoundingVolume>), // May require Box<BoundingVolume> if recursive
    // HalfSpace(BoundingHalfSpace),
}
