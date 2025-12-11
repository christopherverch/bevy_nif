use std::io::ErrorKind;

// src/nif/loader.rs
use bevy::asset::RenderAssetUsages;
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
use nif::loader::{NiKey, load_nif_bytes};

pub use nif::loader::Nif;
/// Data extracted from NiSkinInstance and NiSkinData to build the Bevy skeleton.
#[derive(Debug, Clone)]
pub struct NifSkinData {
    pub skeleton_root: NiKey,  // The key of the root bone (NiNode)
    pub bone_keys: Vec<NiKey>, // The array of bone NiKeys (nodes that form the skeleton)
    pub inverse_bind_poses: Handle<SkinnedMeshInverseBindposes>, // The GPU-ready asset
}
/// Minimal data required for Bevy entity creation and hierarchy traversal.
#[derive(Debug, Clone)]
pub struct NifHierarchyMetadata {
    pub transform: Transform,
    pub children: Vec<NiKey>,
    pub name: String,

    // This key ties the spatial block to its functional component in Nif::components.
    pub component_key: NiKey,
}

#[derive(Debug, Clone)]
pub enum NifComponent {
    // For NiTriShape
    Mesh(NifMeshComponent),

    // For blocks that ONLY group or are parents (like a standard NiNode with no children)
    // We only put it here if it has properties/controllers attached that need runtime setup.
    // Otherwise, a simple NiNode is just in the hierarchy map, with no entry here.
    NodeBase,
}
#[derive(Debug, Clone)]
pub struct NifMeshComponent {
    // The actual Bevy asset handle, resolved during the loader's work.
    pub mesh_handle: Handle<Mesh>,

    // The actual Bevy asset handle for the material.
    pub material_handle: Handle<StandardMaterial>,

    /// The NiKey of the linked NiSkinInstance block, if skinning is present.
    pub skin_instance_key: Option<NiKey>,
}

#[derive(Default)]
pub struct NifAssetLoader;

impl AssetLoader for NifAssetLoader {
    type Asset = Nif;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();

        if let Err(e) = reader.read_to_end(&mut bytes).await {
            error!("NifAssetLoader: Failed to read bytes: {:?}", e);
            return Err(e);
        }
        load_nif_bytes(&bytes, load_context)
    }

    fn extensions(&self) -> &[&str] {
        &["nif", "kf"]
    }
}

#[derive(Default)]
pub struct BMPLoader;

impl AssetLoader for BMPLoader {
    type Asset = Image; // It loads Bevy Images
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?; // Propagate IO errors

        // Use the bmp crate to parse
        let bmp_img = bmp::from_reader(&mut std::io::Cursor::new(&bytes)) // Pass slice reference
            .map_err(|e| {
                std::io::Error::new(ErrorKind::Other, format!("BMP parsing error: {:?}", e))
            })?;

        let width = bmp_img.get_width();
        let height = bmp_img.get_height();

        // Convert BMP pixel data (usually BGR) to RGBA8 for Bevy Image
        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let px = bmp_img.get_pixel(x, y);
                // BMP stores BGR, Bevy needs RGBA
                rgba_data.push(px.r);
                rgba_data.push(px.g);
                rgba_data.push(px.b);
                rgba_data.push(255); // Assume fully opaque alpha for BMP
            }
        }

        // Create Bevy Image
        let image = Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba_data,
            // Assume sRGB for color data. Use Rgba8Unorm if it's linear data.
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );

        Ok(image)
    }

    fn extensions(&self) -> &[&str] {
        &["BMP", "bmp"] // Register for uppercase .BMP also
    }
}
