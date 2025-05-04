//! This module defines the core data structures representing parsed NIF file blocks.

// Declare the sub-modules
pub mod animation;
pub mod base;
pub mod effects;
pub mod extra_data;
pub mod geometry;
pub mod parsing;
pub mod properties;
pub mod scene;
pub mod textures;

pub use animation::{
    KeyFloat, KeyQuaternion, KeyType, KeyVec3, MorphTarget, NiGeomMorpherController,
    NiKeyframeController, NiKeyframeData, NiMorphData, Quaternion, TextKey,
};
pub use base::{
    BoundingSphere, Matrix3x3, NiTransform, NifHeader, Plane, RecordLink, Vector2, Vector3, Vector4,
};
pub use effects::{CoordGenType, EffectType, NiDynamicEffect, NiTextureEffect};
pub use extra_data::{NiExtraData, NiTextKeyExtraData};
pub use geometry::{NiGeometryData, NiTriBasedGeomData, NiTriShapeData};
pub use parsing::{ParsedBlock, ParsedNifData};
pub use properties::{
    LightMode, NiAlphaProperty, NiMaterialProperty, NiProperty, NiVertexColorProperty, VertexMode,
};
pub use scene::{NiAVObject, NiNode, NiObjectNET, NiTriShape};
pub use textures::{
    AlphaFormat, ApplyMode, ClampMode, FilterMode, MipMapFormat, NiSourceTexture,
    NiTexturingProperty, NifTextureInfo, PixelLayout, TextureData,
};
