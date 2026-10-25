// Based on https://github.com/Greatness7/tes3/
// rust std imports
use std::io::{Read, Seek};
use std::path::Path;

// external imports
use bevy_asset::{Asset, Handle, LoadContext};
use bevy_log::warn;
use bevy_reflect::TypePath;
use bevy_world_serialization::WorldAsset;
use slotmap::{DenseSlotMap, Key};

// internal imports
use crate::prelude::*;

#[derive(Asset, TypePath, Clone, Debug, Default)]
pub struct Nif {
    pub objects: DenseSlotMap<NiKey, NiType>,
    pub roots: Vec<NiLink<NiObject>>,
    pub all_keyframe_data: HashMap<NiKey, NiKeyframeData>,
    pub all_controller_links: Vec<(NiKey, NiKeyframeController)>,
    pub text_keys: Vec<NiTextKey>,
    /// For the animation setup system to easily find the bone it's looking for
    pub node_names: HashMap<NiKey, String>,
    pub scenes: Vec<Handle<WorldAsset>>,
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

pub fn load_nif_bytes(bytes: &[u8], _load_context: &mut LoadContext<'_>) -> io::Result<Nif> {
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
    let mut all_controller_links = Vec::new();
    let mut all_keyframe_data = HashMap::new();
    let mut all_tked = HashMap::new();
    let mut all_sed = HashMap::new();
    let mut node_names = HashMap::new();

    // populate objects
    // Ordering when creating NiLinks is identical to how the denseslotmap orders created keys
    // NiTypes that the data is used and not stored have an Empty NiType inserted
    // to keep the ordering expected by a NiLink to be preserved.
    for _ in 0..num_objects {
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
            // All other types, store in full with no preprocessing
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
    // Text keys are usually stored on Bip01 or "Root Bone" node, this function extracts the text
    // keys (animation events like footstep sound markers) if it finds any on bip01 or root bone.
    let final_text_keys = 'data_extraction: {
        // Find Bip01, or if not found, search for "Root Bone"
        let target_node_key = node_names
            .iter()
            .find(|(_, name)| *name == "Bip01") // Find Bip01 (Priority 1)
            .or_else(|| node_names.iter().find(|(_, name)| *name == "Root Bone")) // Fallback to Root Bone (Priority 2)
            .map(|(key, _)| key);

        let Some(key) = target_node_key else {
            break 'data_extraction Vec::new();
        };

        // Get the NiNode block using the key
        let Some(root_node) = objects.get(*key) else {
            break 'data_extraction Vec::new();
        };
        let root_node = match root_node {
            NiType::NiNode(node) => node,
            _ => {
                warn!("found the wrong type of root node while loading nif text keys");
                break 'data_extraction Vec::new();
            }
        };
        // Start the traversal from the node's extra_data
        let mut current_link_key = root_node.extra_data.key;

        // Loop through the extra data chain
        while !current_link_key.is_null() {
            let key_to_lookup = current_link_key;
            // Check for NiTextKeyExtraData (the target block)
            if let Some(tked) = all_tked.get(&key_to_lookup) {
                // Found the correct TextKey block linked from Bip01/Root Bone.
                break 'data_extraction tked.keys.clone();
            }

            // Follow the next link in the chain:
            current_link_key = if let Some(tked) = all_tked.get(&key_to_lookup) {
                // This is the TextKey block we just checked (if it wasn't the target, we shouldn't be here)
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
        all_keyframe_data,
        all_controller_links,
        text_keys: final_text_keys,
        node_names,
        scenes: Vec::new(),
    })
}

pub fn resolve_nif_path(nif_path: &str) -> String {
    // Basic cleanup - Needs proper implementation
    let cleaned = nif_path.trim().replace('\\', "/");
    if !cleaned.starts_with("textures/") && !cleaned.is_empty() {
        format!("textures/{}", cleaned)
    } else {
        cleaned
    }
}
