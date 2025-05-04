#![allow(dead_code)]
use super::NiProperty;
use super::base::{BoundingSphere, Vector2, Vector3, Vector4};
use std::fmt::Debug;
use std::ops::Deref;

// Base class for geometry data blocks
#[derive(Debug, Clone, Default)]
pub struct NiGeometryData {
    // Note: No NiObjectNET base here normally
    pub num_vertices: u16,
    pub has_vertices: bool,
    pub vertices: Option<Vec<Vector3>>, // Make Option<> for conditional reading
    pub has_normals: bool,
    pub normals: Option<Vec<Vector3>>,
    pub bounding_sphere: BoundingSphere,
    pub has_vertex_colors: bool,
    pub vertex_colors: Option<Vec<Vector4>>, // RGBA
    pub num_uv_sets: u16,                    // Derived from flags in 4.0.0.2
    pub uv_sets: Vec<Vec<Vector2>>,          // List of UV sets (each set is a Vec)
}

// Inherits (conceptually) from NiGeometryData
#[derive(Debug, Clone, Default)]
pub struct NiTriBasedGeomData {
    pub geom_base: NiGeometryData, // Composition
    pub num_triangles: u16,
}

// Specific triangle soup data
#[derive(Debug, Clone, Default)]
pub struct NiTriShapeData {
    pub tri_base: NiTriBasedGeomData, // Composition
    pub num_triangle_points: u32,     // Read from file, should == num_triangles * 3
    pub triangles: Vec<u16>,          // Triangle indices (vertex indices)
    pub num_match_groups: u16,
    pub match_groups: Vec<Vec<u16>>,
}

#[derive(Debug, Clone)]
pub struct NiWireframeProperty {
    pub base_property: NiProperty, // Contains NiObjectNET and NiProperty data
    /// Flags specific to NiWireframeProperty. Bit 0 is typically the enable flag.
    pub wire_flags: u16,
}

impl NiWireframeProperty {
    /// Helper method to check if wireframe rendering is enabled (usually bit 0)
    pub fn is_wireframe_enabled(&self) -> bool {
        // Check bit 0 of the wire_flags
        (self.wire_flags & 0x0001) != 0
    }
}

// --- Deref Implementations ---
impl Deref for NiTriBasedGeomData {
    type Target = NiGeometryData;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.geom_base
    }
}
impl Deref for NiTriShapeData {
    type Target = NiTriBasedGeomData;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.tri_base
    }
}
