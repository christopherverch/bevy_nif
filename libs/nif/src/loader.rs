// rust std imports
use std::io::{Read, Seek};
use std::path::Path;

use bevy::asset::{Asset, Handle, LoadContext, RenderAssetUsages};
use bevy::log::{error, info, warn};
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::reflect::TypePath;
// external imports
use slotmap::{DenseSlotMap, Key, new_key_type};

// internal imports
use crate::prelude::*;

new_key_type! { pub struct NiKey; }
// A single type to represent any generated Bevy asset handle

#[derive(Clone, Debug)]
pub enum ConsumedNiType {
    NiTriShapeData(Handle<Mesh>),
}
#[derive(Asset, TypePath, Clone, Debug, Default)]
pub struct Nif {
    pub objects: DenseSlotMap<NiKey, NiType>,
    pub roots: Vec<NiLink<NiObject>>,
    pub block_assets: HashMap<NiKey, ConsumedNiType>,
    pub all_keyframe_data: HashMap<NiKey, NiKeyframeData>,
    pub all_controller_links: Vec<(NiKey, NiKeyframeController)>,
    pub text_keys: Vec<NiTextKey>,
    pub node_names: HashMap<NiKey, String>,
}

pub const HEADER: [u8; 40] = *b"NetImmerse File Format, Version 4.0.0.2\n";
pub const VERSION: u32 = 0x4000002;

pub fn load_nif_from_path(
    path: impl AsRef<Path>,
    load_context: &mut LoadContext<'_>,
) -> io::Result<Nif> {
    load_path(path, load_context)
}

pub fn from_path_offset(
    path: impl AsRef<Path>,
    offset: u64,
    size: usize,
    load_context: &mut LoadContext<'_>,
) -> io::Result<Nif> {
    let mut file = std::fs::File::open(path)?;
    file.seek(io::SeekFrom::Start(offset))?;

    let mut bytes = vec![0; size];
    file.read_exact(&mut bytes)?;

    load_nif_bytes(&bytes, load_context)
}

pub fn load_path(path: impl AsRef<Path>, load_context: &mut LoadContext<'_>) -> io::Result<Nif> {
    load_nif_bytes(&std::fs::read(path)?, load_context)
}

pub fn from_bytes(bytes: &[u8], load_context: &mut LoadContext<'_>) -> io::Result<Nif> {
    load_nif_bytes(bytes, load_context)
}

/// The intended design of this function is:
/// Read the nif file, setting up Bevy assets for any NiType that can be made into a bevy asset
/// and store it in the denseslotmap as NiType::Empty. All other structural NiTypes are stored
/// as-is. Any time the bevy system that actually uses the Nif asset (spawning system) comes across NiType::Empty,
/// it should check the block_data hashmap with the corresponding key, and check which type was consumed
/// and get the bevy asset handles through that
pub fn load_nif_bytes(bytes: &[u8], load_context: &mut LoadContext<'_>) -> io::Result<Nif> {
    dbg!(load_context.path());
    let mut stream = Reader::new(bytes);
    // validate header
    let header: [u8; 40] = stream.load()?;
    if header != HEADER {
        return Reader::error("Invalid NIF Header");
    }

    // validate version
    let version: u32 = stream.load()?;
    if version != VERSION {
        return Reader::error("Invalid NIF Version");
    }

    // allocate objects
    let mut objects = DenseSlotMap::default();
    let num_objects = stream.load_as::<u32, usize>()?;
    objects.reserve(num_objects);
    let mut block_assets = HashMap::new();
    // populate objects
    let mut all_controller_links = Vec::new();
    let mut all_keyframe_data = HashMap::new();
    let mut all_tked = HashMap::new();
    let mut all_sed = HashMap::new();
    let mut node_names = HashMap::new();
    for i in 0..num_objects {
        let ni_type: NiType = stream.load()?;
        match ni_type {
            NiType::NiNode(node) => {
                let name = node.name.clone();
                let key = objects.insert(NiType::NiNode(node));
                node_names.insert(key, name);
            }
            NiType::NiTriShape(trishape) => {
                let name = trishape.name.clone();
                let key = objects.insert(NiType::NiTriShape(trishape));
                node_names.insert(key, name);
            }
            NiType::NiTriShapeData(data) => {
                if let Some(mesh) = convert_nif_mesh(data) {
                    let handle = load_context.add_labeled_asset(format!("mesh_{}", i), mesh);
                    // Inserting into this vec first because we don't have NiKey until we insert it
                    // into the slotmap, which would mean it's moved so we can't match on it.
                    let key: NiKey = objects.insert(NiType::Empty);
                    block_assets.insert(key, ConsumedNiType::NiTriShapeData(handle));
                }
            }
            NiType::NiKeyframeData(kfd) => {
                let key: NiKey = objects.insert(NiType::Empty);
                all_keyframe_data.insert(key, kfd);
            }
            NiType::NiKeyframeController(kfc) => {
                let target_key = kfc.target.key;
                all_controller_links.push((target_key, kfc));
                objects.insert(NiType::Empty);
            }
            NiType::NiTextKeyExtraData(tked) => {
                let key: NiKey = objects.insert(NiType::Empty);
                all_tked.insert(key, tked);
            }
            NiType::NiStringExtraData(sed) => {
                let key: NiKey = objects.insert(NiType::Empty);
                all_sed.insert(key, sed);
            }

            _ =>
            // All NiTypes that can't just create assets and then be discarded, store them
            // in full
            {
                objects.insert(ni_type);
            }
        }
    }

    // allocate roots
    let mut roots = Vec::new();
    let num_roots = stream.load_as::<u32, usize>()?;
    roots.reserve(num_roots);

    // populate roots
    for _ in 0..num_roots {
        roots.push(stream.load()?);
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
        let Some(root_node) = objects.get(*key) else {
            break 'data_extraction Vec::new();
        };
        let root_node = match root_node {
            NiType::NiNode(node) => node,
            _ => {
                dbg!("found the wrong type of root node");
                break 'data_extraction Vec::new();
            }
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

    Ok(Nif {
        objects,
        roots,
        block_assets,
        all_keyframe_data,
        all_controller_links,
        text_keys: final_text_keys,
        node_names,
    })
}

pub fn nif_depth_first_iter(nif: &Nif) -> impl Iterator<Item = (NiKey, &NiType)> {
    let mut seen = HashSet::new();
    let mut keys = Vec::new();
    nif.roots.visitor(&mut |key| keys.push(key));

    std::iter::from_fn(move || {
        while let Some(key) = keys.pop() {
            if !key.is_null() && seen.insert(key) {
                if let Some(object) = nif.objects.get(key) {
                    object.visitor(&mut |key| keys.push(key));
                    return Some((key, object));
                }
            }
        }
        None
    })
}
pub fn resolve_nif_path(nif_path: &str) -> String {
    // Basic cleanup - Needs proper implementation!
    let cleaned = nif_path.trim().replace('\\', "/");
    if !cleaned.starts_with("textures/") && !cleaned.is_empty() {
        format!("textures/{}", cleaned)
    } else {
        cleaned
    }
}

pub fn convert_nif_mesh(data: NiTriShapeData) -> Option<Mesh> {
    // TODO:: not sure what to do with shared normals
    let NiTriShapeData {
        base,
        triangles,
        shared_normals,
    } = data;
    let NiTriBasedGeomData { base } = base;
    let vertices = base.vertices;
    let normals = base.normals;
    let uvs = base.uv_sets;
    let indices = triangles;
    let final_mesh_opt: Option<Mesh>;
    if !normals.is_empty() {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices);

        // Create the Bevy Mesh
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals); // MOVE

        // Insert UVs if they exist
        if !uvs.is_empty() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs); // MOVE
        }

        // Insert the final flat indices
        mesh.insert_indices(Indices::U16(indices)); // MOVE

        final_mesh_opt = Some(mesh);
    } else {
        // Fallback: Need to call the helper function, which must be updated
        // to accept the new Bevy array types (Vec<[f32; 3]>).

        // NOTE: The helper must either accept the data by value or clone internally.
        // We pass references to the moved data's components before they are consumed below.
        final_mesh_opt = create_mesh_with_flat_normals(
            // Pass the data that contains the vertex position info
            &vertices,
            &indices,
            if uvs.is_empty() { None } else { Some(&uvs) },
        );
    }
    // NIF uses u16 indices

    // TODO: Add vertex colors (Mesh::ATTRIBUTE_COLOR) if data.has_vertex_colors is true

    // Potential Coordinate System Conversion:
    // If NIF is Z-up, Left-handed and Bevy is Y-up, Right-handed,
    // you might need to swizzle/negate vertex positions, normals, and adjust transforms.
    if let Some(mesh) = final_mesh_opt {
        Some(mesh)
    } else {
        None
    }
}
fn create_mesh_with_flat_normals(
    original_vertices_nif: &Vec<[f32; 3]>,
    original_indices: &[u16],
    original_uvs: Option<&Vec<[f32; 2]>>, // Pass converted UVs if available
) -> Option<Mesh> {
    let vertex_count = original_vertices_nif.len();
    if vertex_count == 0 {
        warn!("Cannot compute flat normals: No vertices provided.");
        return None;
    }
    if original_indices.is_empty() {
        warn!("Cannot compute flat normals: No indices provided.");
        return None;
    }
    if original_indices.len() % 3 != 0 {
        warn!(
            "Cannot compute flat normals: Index count ({}) is not a multiple of 3.",
            original_indices.len()
        );
        return None;
    }

    let num_triangles = original_indices.len() / 3;
    let new_vertex_count = num_triangles * 3;

    // Initialize buffers for the new mesh data
    let mut final_vertices: Vec<[f32; 3]> = Vec::with_capacity(new_vertex_count);
    let mut final_normals: Vec<[f32; 3]> = Vec::with_capacity(new_vertex_count);
    let mut final_indices: Vec<u16> = Vec::with_capacity(new_vertex_count);
    // Only create UV buffer if original UVs were present
    let mut final_uvs: Option<Vec<[f32; 2]>> =
        original_uvs.map(|_| Vec::with_capacity(new_vertex_count));

    for i in 0..num_triangles {
        // Get original vertex indices for this triangle
        let idx0_u16 = original_indices[i * 3];
        let idx1_u16 = original_indices[i * 3 + 1];
        let idx2_u16 = original_indices[i * 3 + 2];

        let idx0 = idx0_u16 as usize;
        let idx1 = idx1_u16 as usize;
        let idx2 = idx2_u16 as usize;

        // --- Bounds Checking ---
        if idx0 >= vertex_count || idx1 >= vertex_count || idx2 >= vertex_count {
            warn!(
                "Skipping triangle {} due to out-of-bounds index (Indices: {}, {}, {}; Vertex Count: {}).",
                i, idx0_u16, idx1_u16, idx2_u16, vertex_count
            );
            // To keep subsequent indices correct, we should perhaps push *something*
            // or adjust the final index buffer logic. For simplicity now, we skip.
            // This *could* lead to issues if not handled carefully later.
            // A safer approach might be to push degenerate data or filter indices beforehand.
            continue;
        }

        // Get vertex positions
        let v0 = Vec3::from_array(original_vertices_nif[idx0]);
        let v1 = Vec3::from_array(original_vertices_nif[idx1]);
        let v2 = Vec3::from_array(original_vertices_nif[idx2]);

        // Calculate face normal
        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        let face_normal = edge1.cross(edge2);

        // Normalize, handling degenerate triangles (use Y-up as default)
        let normalized_normal_array = face_normal.try_normalize().unwrap_or(Vec3::Y).to_array();

        // Add duplicated vertex positions
        final_vertices.push(v0.to_array());
        final_vertices.push(v1.to_array());
        final_vertices.push(v2.to_array());

        // Add the same face normal for all 3 vertices of this triangle
        final_normals.push(normalized_normal_array);
        final_normals.push(normalized_normal_array);
        final_normals.push(normalized_normal_array);

        // Add sequential indices for the new vertices
        let base_index = (i * 3) as u16; // Assuming less than 65536 triangles per original mesh part
        final_indices.push(base_index);
        final_indices.push(base_index + 1);
        final_indices.push(base_index + 2);

        // Duplicate UVs if they exist
        if let Some(ref mut uvs_out) = final_uvs {
            if let Some(uvs_in) = original_uvs {
                // Check bounds for original UVs as well
                if idx0 < uvs_in.len() && idx1 < uvs_in.len() && idx2 < uvs_in.len() {
                    uvs_out.push(uvs_in[idx0]);
                    uvs_out.push(uvs_in[idx1]);
                    uvs_out.push(uvs_in[idx2]);
                } else {
                    warn!(
                        "Missing UV data for original indices in triangle {}, using default [0,0].",
                        i
                    );
                    uvs_out.push([0.0, 0.0]);
                    uvs_out.push([0.0, 0.0]);
                    uvs_out.push([0.0, 0.0]);
                }
            }
        }
    }

    // --- Final Check ---
    if final_vertices.len() != final_normals.len() {
        error!(
            "Manual flat normal calculation resulted in mismatch! Verts: {}, Norms: {}",
            final_vertices.len(),
            final_normals.len()
        );
        // This shouldn't happen with this logic unless the continue above caused issues
        return None;
    }
    if let Some(ref uvs) = final_uvs {
        if final_vertices.len() != uvs.len() {
            error!(
                "Manual flat normal calculation resulted in UV mismatch! Verts: {}, UVs: {}",
                final_vertices.len(),
                uvs.len()
            );
            // Handle potential UV mismatch if necessary
        }
    }

    // Create the Bevy Mesh using the generated data
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, final_vertices) // Use new vertices
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, final_normals); // Use new normals

    if let Some(final_uvs_vec) = final_uvs {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, final_uvs_vec);
    }

    mesh.insert_indices(Indices::U16(final_indices)); // Use new sequential indices

    info!(
        "Successfully computed flat normals. Final vertex count: {}",
        mesh.count_vertices()
    );

    Some(mesh)
}
