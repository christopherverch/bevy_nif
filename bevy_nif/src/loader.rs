use bevy_asset::RenderAssetUsages;
use bevy_asset::{AssetLoader, LoadContext, io::Reader};
use bevy_camera::visibility::Visibility;
use bevy_ecs::entity::MapEntities;
use bevy_ecs::reflect::{ReflectComponent, ReflectMapEntities};
use bevy_ecs::world::World;
use bevy_ecs::{component::Component, entity::Entity, hierarchy::ChildOf};
use bevy_image::Image;
use bevy_log::error;
use bevy_reflect::{Reflect, TypePath};
use bevy_render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_transform::components::Transform;
use bevy_world_serialization::WorldAsset;
use image_dds::ddsfile::Dds;
use nif::NiKey;
pub use nif::loader::Nif;
use nif::loader::load_nif_bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, ErrorKind};

use crate::loader_spawners::spawn_nif_node_recursive;
use crate::nif_animation::animation_setup_system::setup_animations_new;
use crate::skeleton::Skeleton;

#[derive(Default, TypePath)]
pub struct NifAssetLoader;
#[derive(Serialize, Deserialize, Default, Clone, Hash, PartialEq, Eq)]
pub struct NifSettings {
    pub skeleton_id: Option<u64>,
    pub target_skeleton_id: Option<u64>,
    /// Whether to mirror all the trishape translations and their meshes/normals
    pub mirror_trishapes: bool,
}
impl AssetLoader for NifAssetLoader {
    type Asset = WorldAsset;
    type Settings = NifSettings;
    type Error = std::io::Error;
    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &NifSettings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<WorldAsset, Self::Error> {
        let mut bytes = Vec::new();
        dbg!(settings.mirror_trishapes);

        if let Err(e) = reader.read_to_end(&mut bytes).await {
            error!("NifAssetLoader: Failed to read bytes: {:?}", e);
            return Err(e);
        }
        let nif = load_nif_bytes(&bytes, load_context)?;
        let mut skeleton_opt = if let Some(skeleton_id) = settings.skeleton_id {
            Some(Skeleton {
                skeleton_id,
                ..Default::default()
            })
        } else {
            None
        };
        let mut spawn_context = SpawnContext {
            already_spawned_nodes: HashMap::default(),
            nif_node_index: NifNodeIndex::default(),
            ninodes_with_bvs: Vec::new(),
            mirror_trishapes: settings.mirror_trishapes,
            target_skeleton_id_opt: settings.target_skeleton_id,
        };
        let mut world = World::default();
        let root_entity = world
            .spawn((Transform::default(), Visibility::Inherited))
            .id();
        for current_node in nif.roots.iter() {
            let root_node_entity = world
                .spawn((
                    Transform::default(),
                    Visibility::Inherited,
                    ChildOf(root_entity),
                ))
                .id();

            // Pass `world` and `load_context` down to your recursive helper
            spawn_nif_node_recursive(
                &nif,
                &mut spawn_context,
                load_context,
                current_node.key,
                root_node_entity,
                None,
                skeleton_opt.as_mut(),
                &mut world,
                false,
                false,
            );
        }
        if !spawn_context.ninodes_with_bvs.is_empty() {
            // TODO:: directly spawn the bvs
            /*
            world
                .entity_mut(root_entity)
                .insert(NeedsNifPhysics(spawn_context.ninodes_with_bvs));
            */
        }

        world
            .entity_mut(root_entity)
            .insert(spawn_context.nif_node_index);

        if let Some(skeleton) = skeleton_opt {
            setup_animations_new(root_entity, &nif, &skeleton, load_context, &mut world);
            world.entity_mut(root_entity).insert(skeleton);
        }
        let world_asset = WorldAsset::new(world);
        Ok(world_asset)
    }

    fn extensions(&self) -> &[&str] {
        &["nif", "kf"]
    }
}

#[derive(Component, Default, Reflect, MapEntities)]
#[reflect(Component, MapEntities)]
pub struct NifNodeIndex {
    /// For specific, named parts (e.g., "NifScene", "LeftArm")
    /// u64 is the hashed string for smaller storage size
    pub named_nodes: HashMap<u64, Entity>,

    /// all spawned trishape entities since these are iterated over in bulk in attach_parts
    #[entities] // Tells the derived mapper to map entities inside this collection
    pub tri_shapes: Vec<Entity>,
}
pub(crate) struct SpawnContext {
    /// In case of a circular dependency
    pub(crate) already_spawned_nodes: HashMap<NiKey, Entity>,
    pub(crate) nif_node_index: NifNodeIndex,
    pub(crate) ninodes_with_bvs: Vec<(Entity, NiKey)>,
    pub(crate) mirror_trishapes: bool,
    pub(crate) target_skeleton_id_opt: Option<u64>,
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
        reader.read_to_end(&mut bytes).await?;

        // parse with bmp crate
        let bmp_img = bmp::from_reader(&mut std::io::Cursor::new(&bytes)).map_err(|e| {
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
        &["BMP", "bmp"]
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

        // Parse container via ddsfile
        let dds = Dds::read(&mut Cursor::new(&bytes)).map_err(|e| {
            std::io::Error::new(ErrorKind::Other, format!("DDS parse error: {:?}", e))
        })?;

        // Decode mip 0 into an RgbaImage
        let rgba_image = image_dds::image_from_dds(&dds, 0).map_err(|e| {
            std::io::Error::new(ErrorKind::Other, format!("DDS decode error: {:?}", e))
        })?;

        let width = rgba_image.width();
        let height = rgba_image.height();
        let rgba_data = rgba_image.into_raw();

        // If the header width/height multiplication doesn't match
        // the actual byte length (due to block padding), we override the dimensions
        // or let Bevy take the exact buffer length safely.
        let expected_len = (width * height * 4) as usize;
        let actual_len = rgba_data.len();

        let (final_width, final_height) = if expected_len != actual_len {
            // If there's padding mismatch, recalculate height based on actual buffer bytes
            let corrected_height = (actual_len / 4) / width as usize;
            (width, corrected_height as u32)
        } else {
            (width, height)
        };

        // Create Bevy Image with correct size and data length
        let image = Image::new(
            Extent3d {
                width: final_width,
                height: final_height,
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
