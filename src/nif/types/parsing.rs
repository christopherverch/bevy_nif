#![allow(dead_code)]
use super::animation::{
    NiGeomMorpherController, NiKeyframeController, NiKeyframeData, NiMorphData, NiSkinData,
    NiSkinInstance,
};
use super::base::NifHeader;
use super::effects::NiTextureEffect;
use super::extra_data::NiTextKeyExtraData;
use super::geometry::{NiTriShapeData, NiWireframeProperty};
use super::properties::{NiAlphaProperty, NiMaterialProperty, NiVertexColorProperty};
use super::scene::{NiNode, NiTriShape};
use super::textures::NiSourceTexture;
use super::textures::NiTexturingProperty; // Corrected path
use bevy::prelude::*;
use bevy::reflect::TypePath;
use std::fmt::Debug;

#[derive(Asset, Clone, Debug, TypePath)]
pub enum ParsedBlock {
    Node(NiNode),
    TriShape(NiTriShape),
    AlphaProperty(NiAlphaProperty),
    TexturingProperty(NiTexturingProperty),
    SourceTexture(NiSourceTexture),
    MaterialProperty(NiMaterialProperty),
    TriShapeData(NiTriShapeData),
    KeyframeController(NiKeyframeController),
    KeyframeData(NiKeyframeData),
    TextureEffect(NiTextureEffect),
    TextKeyExtraData(NiTextKeyExtraData),
    VertexColorProperty(NiVertexColorProperty),
    GeomMorpherController(NiGeomMorpherController),
    MorphData(NiMorphData),
    SkinInstance(NiSkinInstance),
    SkinData(NiSkinData),
    WireframeProperty(NiWireframeProperty),
    // Add other variants as needed
    Unknown(String), // Stores the type name of the unknown block
}

#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct ParsedNifData {
    pub header: NifHeader,
    pub blocks: Vec<ParsedBlock>,
}
