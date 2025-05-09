#![allow(dead_code)]
use super::base::RecordLink;
use super::properties::NiProperty;
use super::scene::NiObjectNET;
use std::fmt::Debug;
use std::ops::Deref;

#[derive(Debug, Default, Clone)] // Added Clone
pub struct NifTextureInfo {
    pub base_texture_path: Option<String>,
    pub base_uv_set: u32,
    // Add other texture types as needed:
    // bump_texture_path: Option<String>,
    // glow_texture_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PixelLayout {
    #[default]
    Palettized8, // PIX_LAY_PALETTISED8, Seems most common? Check NifXML/common usage
    HighColor16, // PIX_LAY_HIGH_COLOR_16
    TrueColor32, // PIX_LAY_TRUE_COLOR_32
    Compressed,  // PIX_LAY_COMPRESSED
    Bumpmap,     // PIX_LAY_BUMPMAP
    Palettized4, // PIX_LAY_PALETTISED4
    Default,     // PIX_LAY_DEFAULT
    Unknown(u32),
}
impl From<u32> for PixelLayout {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Palettized8,
            1 => Self::HighColor16,
            2 => Self::TrueColor32,
            3 => Self::Compressed,
            4 => Self::Bumpmap,
            5 => Self::Palettized4,
            6 => Self::Default,
            _ => Self::Unknown(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MipMapFormat {
    No,  // MIP_FMT_NO (Generate no mipmaps)
    Yes, // MIP_FMT_YES (Generate mipmaps)
    #[default]
    Default, // MIP_FMT_DEFAULT (Use renderer default)
    Unknown(u32),
}
impl From<u32> for MipMapFormat {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::No,
            1 => Self::Yes,
            2 => Self::Default,
            _ => Self::Unknown(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlphaFormat {
    #[default]
    None, // ALPHA_NONE (No alpha)
    Binary,  // ALPHA_BINARY (1-bit alpha)
    Smooth,  // ALPHA_SMOOTH (Full alpha)
    Default, // ALPHA_DEFAULT (Use renderer default)
    Unknown(u32),
}
impl From<u32> for AlphaFormat {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Binary,
            2 => Self::Smooth,
            3 => Self::Default,
            _ => Self::Unknown(value),
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct NiSourceTexture {
    pub net_base: NiObjectNET,       // Composition
    pub use_external: bool,          // Flag: Is image data external (DDS file)?
    pub file_name: Option<String>, // External file name (DDS path), present only if use_external is true
    pub pixel_data_link: RecordLink, // Link to NiPixelData record, present only if use_external is false AND internal flag is true
    pub pixel_layout: PixelLayout,   // Format prefs
    pub use_mipmaps: MipMapFormat,   // Format prefs
    pub alpha_format: AlphaFormat,   // Format prefs
    pub is_static: bool,             // Flag: Is texture static? (Usually true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApplyMode {
    #[default]
    Replace,
    Decal,
    Modulate,
    Hilight,
    Hilight2,
    Unknown(u32),
}
impl From<u32> for ApplyMode {
    fn from(value: u32) -> Self {
        match value {
            0 => ApplyMode::Replace,
            1 => ApplyMode::Decal,
            2 => ApplyMode::Modulate,
            3 => ApplyMode::Hilight,
            4 => ApplyMode::Hilight2,
            // Capture unknown values
            other => ApplyMode::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClampMode {
    // Wrap modes
    #[default]
    ClampSClampT,
    ClampSWrapT,
    WrapSClampT,
    WrapSWrapT,
    Unknown(u32),
}
impl From<u32> for ClampMode {
    fn from(value: u32) -> Self {
        match value & 0b11 {
            // Keep using lower 2 bits for v4.0.0.2 logic
            0 => ClampMode::ClampSClampT,
            1 => ClampMode::ClampSWrapT,
            2 => ClampMode::WrapSClampT,
            3 => ClampMode::WrapSWrapT,
            // This case _should_ be unreachable if masking with 0b11, but handle defensively
            other => ClampMode::Unknown(other), // Or perhaps a specific error/default?
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterMode {
    // Texture filtering
    #[default]
    Nearest,
    Linear,
    NearestMipNearest,
    NearestMipLinear,
    LinearMipNearest,
    LinearMipLinear,
    Unknown(u32),
}
impl From<u32> for FilterMode {
    fn from(value: u32) -> Self {
        match value {
            0 => FilterMode::Nearest,
            1 => FilterMode::Linear,
            2 => FilterMode::NearestMipNearest,
            3 => FilterMode::NearestMipLinear,
            4 => FilterMode::LinearMipNearest,
            5 => FilterMode::LinearMipLinear,
            // Capture unknown values
            other => FilterMode::Unknown(other),
        }
    }
}

// Nested struct for individual texture slots
#[derive(Debug, Clone, Default)]
pub struct TextureData {
    pub has_texture: bool,          // Whether this slot is used
    pub source_texture: RecordLink, // Link to NiSourceTexture
    pub clamp_mode: ClampMode,
    pub filter_mode: FilterMode,
    pub uv_set: u32,
    // PS2 L (2 bytes), PS2 K (2 bytes) - Read but unused
    // Unknown short (2 bytes) - Read but unused
    // Texture Transform not read in v4.0.0.2
}

// NiTexturingProperty struct definition
#[derive(Debug, Clone, Default)]
pub struct NiTexturingProperty {
    pub property_base: NiProperty, // Composition
    pub flags: u16,                // Texture flags (e.g., multi-texture enable)
    pub apply_mode: ApplyMode,
    pub texture_count: u32, // Number of textures read from file (for info)
    // Uses a fixed-size array for Morrowind's common slots for simplicity,
    // or use Vec<TextureData> if counts vary wildly. Let's use array.
    pub base_texture: Option<TextureData>,
    pub dark_texture: Option<TextureData>,
    pub detail_texture: Option<TextureData>,
    pub gloss_texture: Option<TextureData>,
    pub glow_texture: Option<TextureData>,
    pub bump_map_texture: Option<TextureData>,
    pub normal_texture: Option<TextureData>, // Not typically used in MW base game NIFs
    pub decal_0_texture: Option<TextureData>,
    // Bump map specific fields (read only if bump map slot is enabled)
    pub bump_map_luma_scale: f32,
    pub bump_map_luma_offset: f32,
    pub bump_map_matrix: [f32; 4], // M11, M12, M21, M22
}

// --- Deref Implementations ---
impl Deref for NiSourceTexture {
    type Target = NiObjectNET;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.net_base
    }
}

impl Deref for NiTexturingProperty {
    type Target = NiProperty;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.property_base
    }
}
