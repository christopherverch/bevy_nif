use bevy_asset::RenderAssetUsages;
use bevy_asset::{AssetLoader, LoadContext, io::Reader};
use bevy_image::Image;
use bevy_log::error;
use bevy_reflect::TypePath;
use bevy_render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use image_dds::ddsfile::Dds;
pub use nif::loader::Nif;
use nif::loader::load_nif_bytes;
use std::io::{Cursor, ErrorKind};

#[derive(Default, TypePath)]
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

#[derive(Default, TypePath)]
pub struct BMPLoader;

impl AssetLoader for BMPLoader {
    type Asset = Image;
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
#[derive(Default, TypePath)]
pub struct DDSLoader;

impl AssetLoader for DDSLoader {
    type Asset = Image;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        // 1. Parse the raw bytes into a Dds struct using the `ddsfile` crate
        let dds = Dds::read(&mut Cursor::new(&bytes)).map_err(|e| {
            std::io::Error::new(
                ErrorKind::Other,
                format!("DDS container parse error: {:?}", e),
            )
        })?;

        // 2. Decode the DDS mip level 0 into an uncompressed RgbaImage on the CPU
        let rgba_image = image_dds::image_from_dds(&dds, 0).map_err(|e| {
            std::io::Error::new(ErrorKind::Other, format!("DDS decoding error: {:?}", e))
        })?;

        let width = rgba_image.width();
        let height = rgba_image.height();
        let rgba_data = rgba_image.into_raw(); // Already an RgbaImage, get underlying Vec<u8>

        // 3. Create the Bevy Image using plain uncompressed pixels
        let image = Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba_data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );

        Ok(image)
    }

    fn extensions(&self) -> &[&str] {
        &["DDS", "dds"]
    }
}
