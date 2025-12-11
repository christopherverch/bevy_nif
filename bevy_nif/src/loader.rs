use std::io::ErrorKind;

// src/nif/loader.rs
use bevy::asset::RenderAssetUsages;
use bevy::render::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
use nif::loader::{NiKey, load_nif_bytes};

pub use nif::loader::Nif;
#[allow(dead_code)]
/*
#[derive(Asset, TypePath, Debug)]
pub struct Nif {
    // --- 1. HIERARCHY / SPATIAL DATA (The primary map for recursion) ---
    /// A map holding the MINIMAL data needed to spawn and traverse a Bevy entity.
    /// Only contains entries for blocks that have a transform (NiNode, NiTriShape, NiLight, etc.).
    pub hierarchy: HashMap<NiKey, NifHierarchyMetadata>,

    /// The starting points for the scene graph traversal.
    pub root_keys: Vec<NiKey>,

    // --- 2. FUNCTIONAL COMPONENTS (Sparse data for specific block types) ---
    /// A single map holding the functional components. This is looked up after
    /// the hierarchy metadata and contains the fully resolved Bevy handles.
    pub components: HashMap<NiKey, NifComponent>,

    // --- 3. ANIMATION DATA (Flattened and pre-processed) ---
    /// Map of all NiNode keys (bones) to their names for skeleton/animation linking.
    pub node_names: HashMap<NiKey, String>,

    /// Pre-processed and simplified animation data (e.g., Bevy curves).
    /// This replaces the need for NiKeyframeData, NiFloatData, etc.
    pub animation_clips: HashMap<String, Handle<AnimationClip>>,

    /// Sparse map holding the necessary links/metadata to build the SkinnedMeshBundle.
    pub skin_data: HashMap<NiKey, NifSkinData>,
}
*/
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
        /*
        // Temporary staging areas (for Phase 2 input)
        let mut trishape_staging: HashMap<NiKey, &NiTriShape> = HashMap::new();
        let mut node_staging: HashMap<NiKey, &NiNode> = HashMap::new();

        // Lookup maps for handles and raw property data
        let mut mesh_handles: HashMap<NiKey, Handle<Mesh>> = HashMap::new();
        let mut raw_material_props: HashMap<NiKey, NiMaterialProperty> = HashMap::new();
        let mut texture_info_map: HashMap<NiKey, NifTextureInfo> = HashMap::new();
        let mut node_names: HashMap<NiKey, String> = HashMap::new();

        // Final Nif Asset Structs that we can partially fill in Pass 1
        let mut final_skin_data: HashMap<NiKey, NifSkinData> = HashMap::new();
        // New map to stage the raw NiSkinInstance blocks (where the NiNode links are)
        let mut skin_instance_staging: HashMap<NiKey, &NiSkinInstance> = HashMap::new();
        // Helper map to store the raw links extracted from NiSkinInstance
        let mut raw_skin_links: HashMap<NiKey, RawSkinLinks> = HashMap::new();
        for (key, ni_type) in stream.objects.iter() {
            match ni_type {
                // --- 1. SPATIAL BLOCKS (STAGE for Phase 2) ---
                tes3::nif::NiType::NiTriShape(trishape) => {
                    // Stage the block itself for processing in Phase 2
                    trishape_staging.insert(key, trishape);
                    // Collect name immediately (used in final NifHierarchyMetadata)
                    node_names.insert(key, trishape.base.base.base.name.clone());
                }
                tes3::nif::NiType::NiNode(node) => {
                    // Stage the block itself for processing in Phase 2
                    node_staging.insert(key, node);
                    // Collect name immediately (used for hierarchy and skeleton linking)
                    node_names.insert(key, node.base.base.name.clone());
                }
                NiType::NiTriShapeData(data) => {
                    if let Some(mesh) = convert_nif_mesh(data) {
                        let label = format!("mesh_{:?}", key);
                        mesh_handles.insert(key, load_context.add_labeled_asset(label, mesh));
                    }
                }
                tes3::nif::NiType::NiSkinData(skin_data) => {
                    // Create the GPU-ready Inverse Bind Pose asset immediately (it's file-data-only)
                    let mut ibp_matrices = Vec::with_capacity(skin_data.bone_data.len());
                    for bone_data in &skin_data.bone_data {
                        // Convert NIF transform -> Bevy Transform -> Bevy Mat4
                        // Assumes bone_data.bone_transform IS the inverse bind pose
                        let bone_transform = Transform {
                            rotation: Quat::from_mat3(&bone_data.rotation),
                            translation: bone_data.translation,
                            scale: Vec3::splat(bone_data.scale),
                        };
                        let bevy_transform = convert_nif_transform(&bone_transform);
                        ibp_matrices.push(bevy_transform.compute_matrix());
                    }
                    // Create and add the asset
                    let ibp_asset = SkinnedMeshInverseBindposes::from(ibp_matrices);
                    let label = format!("ibp_{:?}", key);
                    let ibp_handle = load_context.add_labeled_asset(label, ibp_asset);
                    // Store the final NifSkinData structure for the Nif asset
                    let bone_keys: Vec<NiKey> = skin_data
                        .bone_data
                        .iter()
                        .filter_map(|bone_data| bone_data.bone.as_key()) // Correctly accessing the link inside BoneData
                        .collect();
                    let nif_skin_data = NifSkinData {
                        // We cannot determine skeleton_root here, so we use a placeholder.
                        // It will be updated in Phase 2 when processing NiSkinInstance.
                        skeleton_root: NiKey::null(),
                        bone_keys, // Storing the resolved bone keys
                        inverse_bind_poses: ibp_handle,
                    };
                    final_skin_data.insert(key, nif_skin_data);
                }
                tes3::nif::NiType::NiMaterialProperty(mat_prop) => {
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
                    material.perceptual_roughness = 1.0 - (mat_prop.shine / 100.0).clamp(0.0, 1.0);
                    material.alpha_mode = if mat_prop.alpha < 0.99 {
                        AlphaMode::Blend
                    } else {
                        AlphaMode::Opaque
                    };
                    let label = format!("material_{:?}", key);
                    material_handles.insert(key, load_context.add_labeled_asset(label, material));
                }
                tes3::nif::NiType::NiTexturingProperty(tex_prop) => {
                    let mut tex_info = NifTextureInfo::default();
                    // For now just use the 0 index texture map, TODO::
                    if let Some(Some(TextureMap::Map(map_data))) = tex_prop.texture_maps.get(0) {
                        if let Some((filename, uv_set_index)) =
                            resolve_texture_path(map_data, &stream, key)
                        {
                            // SUCCESS: We found the filename and UV set index
                            tex_info.base_texture_path = Some(filename);
                            tex_info.base_uv_set = uv_set_index as u32;
                        }

                        // The old code inserted texture_info_map inside the check.
                        // It's cleaner to insert only if we actually found something.
                        texture_info_map.insert(key, tex_info);
                    }
                }
                // --- ANIMATION DATA EXTRACTION ---

                // 5. ANIMATION: Keyframe Data (Raw Curves)
                tes3::nif::NiType::NiKeyframeData(kfd) => {
                    // Insert the data, keyed by its own NiKey
                    all_keyframe_data.insert(key, kfd.clone());
                }

                // 6. ANIMATION: Keyframe Controller (Linker)
                tes3::nif::NiType::NiKeyframeController(kfc) => {
                    // Extract the Target Key from the controller's link field
                    let target_key = kfc.target.key;
                    // Store the target key alongside the controller block.
                    all_controller_links.push((target_key, kfc.clone()));
                }

                // 7. ANIMATION: Text Keys (Timing/Events)
                tes3::nif::NiType::NiTextKeyExtraData(tked) => {
                    // Store for post-processing traversal (Step 1)
                    all_tked.insert(key, tked);
                    // REMOVE your previous logic here:
                    // if text_keys.is_empty() { text_keys = tked.keys.clone(); }
                    // We will now calculate text_keys accurately below.
                }

                // 8. ANIMATION: String Extra Data (Needed to traverse the chain)
                tes3::nif::NiType::NiStringExtraData(sed) => {
                    // Store the SED for post-processing traversal (Step 1)
                    all_sed.insert(key, sed);
                }
                _ => {}
            }
        }

        // Bip01 Traversal Logic
        let final_text_keys = 'data_extraction: {
            // 1. Prioritized Node Key Search: Search for "Bip01" first, then "Root Bone".
            let target_node_key = node_names
                .iter()
                .find(|(_, name)| *name == "Bip01") // Find Bip01 (Priority 1)
                .or_else(|| node_names.iter().find(|(_, name)| *name == "Root Bone")) // Fallback to Root Bone (Priority 2)
                .map(|(key, _)| key);

            let Some(key) = target_node_key else {
                break 'data_extraction Vec::new();
            };

            // 2. Get the full NiNode block using the key
            let Some(root_node) = all_nodes.get(key) else {
                break 'data_extraction Vec::new();
            };
            // 3. Start the traversal from the node's extra_data_link
            let mut current_link_key = root_node.extra_data.key;

            // Loop through the extra data chain using NiKey lookups
            while !current_link_key.is_null() {
                let key_to_lookup = current_link_key;
                // Check for NiTextKeyExtraData (the target block)
                if let Some(tked) = all_tked.get(&key_to_lookup) {
                    // Found the correct TextKey block linked from Bip01/Root Bone.
                    break 'data_extraction tked.keys.clone();
                }

                // Follow the next link in the chain:
                current_link_key = if let Some(tked) = all_tked.get(&key_to_lookup) {
                    // This is the TextKey block we *just* checked (if it wasn't the target, we shouldn't be here)
                    tked.base.next.key
                } else if let Some(sed) = all_sed.get(&key_to_lookup) {
                    // Found NiStringExtraData, continue the chain
                    sed.base.next.key
                } else {
                    // Found a block not in our traversal types (chain ends)
                    NiKey::null()
                };

                if current_link_key.is_null() {
                    break; // Chain is broken
                }
            }

            Vec::new() // Traversal failed
        };
        let root_keys: Vec<NiKey> = stream.roots.iter().map(|ni_link| ni_link.key).collect();
        let collision_shapes = find_collision_shapes(&stream);
        let nif = Nif {
            root_keys,
            mesh_handles,
            material_handles,
            texture_info_map,
            collision_shapes,
            node_names,
            all_keyframe_data,
            all_controller_links,
            text_keys: final_text_keys,
        };
        Ok(nif)
            */
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
