// src/nif/loader.rs
use crate::nif::error::ParseError; // Use path via nif module
use crate::nif::parser::start::parse_nif_start; // Use path via nif module
use crate::nif::types::ParsedBlock;
use crate::{NifTextureInfo, ParsedNifData};
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*, // Use prelude for common types
};
use std::collections::HashMap;

use super::helper_funcs::convert_nif_mesh;
#[allow(dead_code)]
#[derive(Asset, TypePath, Debug)]
pub struct Nif {
    /// Handles to all Bevy meshes extracted from NiTriShapeData blocks.
    pub mesh_handles: HashMap<usize, Handle<Mesh>>,
    /// Handles to all Bevy StandardMaterials created from NIF material properties.
    pub material_handles: HashMap<usize, Handle<StandardMaterial>>,
    pub texture_info_map: HashMap<usize, NifTextureInfo>,
    pub raw_parsed: ParsedNifData,
}
#[derive(Default)]
pub struct NifAssetLoader;

impl AssetLoader for NifAssetLoader {
    type Asset = Nif;
    type Settings = ();
    type Error = ParseError;
    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();

        if let Err(e) = reader.read_to_end(&mut bytes).await {
            error!("NifAssetLoader: Failed to read bytes: {:?}", e); // Use error! macro
            return Err(ParseError::Io(e));
        }

        let nif_data = parse_nif_start(&bytes)?;
        let mut block_map: HashMap<usize, &ParsedBlock> = HashMap::new();
        for (index, block) in nif_data.blocks.iter().enumerate() {
            block_map.insert(index, block);
        }
        let mut mesh_handles: HashMap<usize, Handle<Mesh>> = HashMap::new();
        let mut material_handles: HashMap<usize, Handle<StandardMaterial>> = HashMap::new();
        let mut texture_info_map: HashMap<usize, NifTextureInfo> = HashMap::new();

        // --- Pass 1: Create Bevy Assets ---
        for (index, block) in nif_data.blocks.iter().enumerate() {
            match block {
                ParsedBlock::TriShapeData(data) => {
                    if let Some(mesh) = convert_nif_mesh(data) {
                        let label = format!("mesh_{}", index);
                        mesh_handles.insert(index, load_context.add_labeled_asset(label, mesh));
                    }
                }
                ParsedBlock::MaterialProperty(mat_prop) => {
                    let mut material = StandardMaterial::default();
                    material.base_color = Color::srgb(
                        mat_prop.diffuse_color[0],
                        mat_prop.diffuse_color[1],
                        mat_prop.diffuse_color[2],
                    );
                    material.emissive = LinearRgba::rgb(
                        mat_prop.emissive_color[0],
                        mat_prop.emissive_color[1],
                        mat_prop.emissive_color[2],
                    );
                    material.metallic = 0.1;
                    material.perceptual_roughness =
                        1.0 - (mat_prop.glossiness / 100.0).clamp(0.0, 1.0);
                    material.alpha_mode = if mat_prop.alpha < 0.99 {
                        AlphaMode::Blend
                    } else {
                        AlphaMode::Opaque
                    };
                    let label = format!("material_{}", index);
                    material_handles.insert(index, load_context.add_labeled_asset(label, material));
                }
                ParsedBlock::TexturingProperty(tex_prop) => {
                    // This is where we resolve the texture path using the linked NiSourceTexture
                    let mut tex_info = NifTextureInfo::default();

                    // --- Minimal logic for Base Texture ---
                    if let Some(base_tex_data) = &tex_prop.base_texture {
                        if base_tex_data.has_texture {
                            // Check if slot is used
                            if let Some(link_idx) = base_tex_data.source_texture {
                                // Check if link exists
                                // Look up the linked block using the index
                                if let Some(linked_block) = block_map.get(&link_idx) {
                                    // Check if the linked block is indeed an NiSourceTexture
                                    if let ParsedBlock::SourceTexture(src_tex) = linked_block {
                                        // Check if it points to an external file
                                        if src_tex.use_external {
                                            // Get the filename Option<String>
                                            if let Some(filename) = &src_tex.file_name {
                                                // SUCCESS: We found the filename!
                                                tex_info.base_texture_path = Some(filename.clone());
                                                // Also store the UV set index associated with this slot
                                                tex_info.base_uv_set = base_tex_data.uv_set;
                                            } else {
                                                warn!(
                                                    "  TexProp {}: SourceTexture {} is external but has no filename.",
                                                    index, link_idx
                                                );
                                            }
                                        } else {
                                            // Handle internal NiPixelData if necessary (usually not for Morrowind)
                                            warn!(
                                                "  TexProp {}: SourceTexture {} uses internal NiPixelData (unsupported).",
                                                index, link_idx
                                            );
                                        }
                                    } else {
                                        warn!(
                                            "  TexProp {}: Link {} does not point to an NiSourceTexture block.",
                                            index, link_idx
                                        );
                                    }
                                } else {
                                    warn!(
                                        "  TexProp {}: Link {} is invalid (points nowhere).",
                                        index, link_idx
                                    );
                                }
                            } else {
                                warn!(
                                    "  TexProp {}: BaseTexture enabled but has no SourceTexture link.",
                                    index
                                );
                            }
                            texture_info_map.insert(index, tex_info);
                        }
                    }
                }
                _ => {}
            }
        }
        let nif = Nif {
            mesh_handles,
            material_handles,
            texture_info_map,
            raw_parsed: nif_data,
        };
        Ok(nif)
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
    type Error = ParseError;

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
            .map_err(|e| ParseError::InvalidData(format!("BMP parsing error: {:?}", e)))?;

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
        &["BMP", "bmp"] // Register for uppercase .BMP
    }
}
