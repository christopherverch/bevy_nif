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
use ddsfile::Dds;
use nif::NiKey;
pub use nif::loader::Nif;
use nif::loader::load_nif_bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, ErrorKind};

use crate::loader_spawners::{NeedsNifSkinMapping, spawn_nif_node_recursive};
use crate::nif_animation::animation_setup_system::setup_animations;
use crate::physics::handle_bvs;
use crate::skeleton::Skeleton;

#[derive(Default, TypePath)]
pub struct NifAssetLoader;
#[derive(Serialize, Deserialize, Default, Clone, Hash, PartialEq, Eq)]
pub struct NifSettings {
    pub is_skeleton: bool,
}

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct NeedsSkeletonMapping;

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
        let scratch_string = String::with_capacity(128);

        if let Err(e) = reader.read_to_end(&mut bytes).await {
            error!("NifAssetLoader: Failed to read bytes: {:?}", e);
            return Err(e);
        }
        let nif = load_nif_bytes(&bytes, load_context)?;
        let mut skeleton_opt = if settings.is_skeleton {
            Some(Skeleton::default())
        } else {
            None
        };
        let mut spawn_context = SpawnContext {
            already_spawned_nodes: HashMap::default(),
            nif_node_index: NifNodeIndex::default(),
            ninodes_with_bvs: None,
            trimeshes_with_colliders: None,
            scratch_string,
        };
        let mut world = World::default();
        let root_entity = world
            .spawn((Transform::default(), Visibility::Inherited))
            .id();
        let mut pending_nif_skins = None;
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
                &mut pending_nif_skins,
                &mut world,
                false,
                false,
            );
        }
        let collider_data_opt = handle_bvs(spawn_context.ninodes_with_bvs, &nif);
        // TODO:: directly spawn the bvs
        /*
        world
            .entity_mut(root_entity)
            .insert(NeedsNifPhysics(spawn_context.ninodes_with_bvs));
        */

        world
            .entity_mut(root_entity)
            .insert(spawn_context.nif_node_index);

        if let Some(skeleton) = skeleton_opt {
            setup_animations(root_entity, &nif, &skeleton, load_context, &mut world);
            world
                .entity_mut(root_entity)
                .insert((skeleton, NeedsSkeletonMapping));
        }
        if let Some(pending_nif_skins) = pending_nif_skins {
            world.entity_mut(root_entity).insert(pending_nif_skins);
        }
        if let Some(collider_data) = collider_data_opt {
            world.entity_mut(root_entity).insert(collider_data);
        }
        if let Some(trimeshes_with_colliders) = spawn_context.trimeshes_with_colliders {
            world
                .entity_mut(root_entity)
                .insert(TrimeshesWithColliders(trimeshes_with_colliders));
        }
        let world_asset = WorldAsset::new(world);
        Ok(world_asset)
    }

    fn extensions(&self) -> &[&str] {
        &["nif", "kf"]
    }
}
#[derive(Component, Default, Debug, Reflect, MapEntities)]
#[reflect(Component, MapEntities)]
pub struct PendingNifSkins {
    #[entities]
    pub pending: Vec<Entity>,
    /// Will be filled in in attach_parts
    pub skeleton_entity: Option<Entity>,
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
    pub(crate) ninodes_with_bvs: Option<Vec<(Entity, NiKey)>>,
    pub(crate) trimeshes_with_colliders: Option<Vec<Entity>>,
    pub(crate) scratch_string: String,
}
#[derive(Component, Default, Debug, Reflect, MapEntities)]
#[reflect(Component, MapEntities)]
pub struct TrimeshesWithColliders(#[entities] pub Vec<Entity>);
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

        let width = dds.get_width();
        let height = dds.get_height();

        // Extract raw bytes for the first mipmap level (mip 0)
        let rgba_data = dds
            .get_data(0)
            .map_err(|e| std::io::Error::new(ErrorKind::Other, format!("DDS data error: {:?}", e)))?
            .to_vec();

        // Check format (supports uncompressed RGBA/BGRA formats)
        // If your files use standard uncompressed RGBA:
        let format = TextureFormat::Rgba8UnormSrgb;

        // If your textures are BGRA instead, change to TextureFormat::Bgra8UnormSrgb

        let expected_len = (width * height * 4) as usize;
        let actual_len = rgba_data.len();

        let final_height = if expected_len != actual_len {
            (actual_len / 4) / width as usize
        } else {
            height as usize
        } as u32;

        let image = Image::new(
            Extent3d {
                width,
                height: final_height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba_data,
            format,
            RenderAssetUsages::default(),
        );

        Ok(image)
    }

    fn extensions(&self) -> &[&str] {
        &["DDS", "dds"]
    }
}
