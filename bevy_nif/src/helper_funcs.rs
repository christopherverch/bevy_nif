use avian3d::parry::utils::hashmap::HashMap;
use bevy_log::warn;
use std::path::{Component, Path};
use std::sync::Mutex;
use std::{env, path::PathBuf};

include!(concat!(env!("OUT_DIR"), "/asset_manifest.rs"));

fn path_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::default()))
}
#[inline]
pub fn clean_path_common(path: &str, out: &mut String) {
    out.clear();
    let trimmed = path.trim();
    out.reserve(trimmed.len());

    for b in trimmed.bytes() {
        if b != b'\\' {
            out.push(b as char);
        } else {
            out.push('/');
        }
    }
}

/// Checks if an asset exists in the compile-time manifest case-insensitively
pub fn check_exists(base: &str) -> bool {
    let normalized = base.replace('\\', "/");
    asset_manifest().contains(CiLookup::from_str(&normalized))
}

/// Resolves an asset path case-insensitively and writes the correct cased path to scratch
pub fn resolve_manifest_path(path_str: &str, scratch: &mut String) -> bool {
    let normalized = path_str.replace('\\', "/");
    if let Some(ci_str) = asset_manifest().get(CiLookup::from_str(&normalized)) {
        scratch.clear();
        scratch.push_str(ci_str.0);
        true
    } else {
        false
    }
}

pub fn resolve_nif_texture_path(nif_path: &str, scratch: &mut String) -> bool {
    // 1. Clean path into scratch to use as our lookup key
    clean_path_common(nif_path, scratch);
    if scratch.is_empty() {
        return false;
    }

    // 2. Check global runtime cache using the cleaned scratch string as a key
    if let Ok(cache) = path_cache().lock() {
        if let Some(cached_opt) = cache.get(scratch.as_str()) {
            if let Some(cached_path) = cached_opt {
                scratch.clear();
                scratch.push_str(cached_path);
                return true;
            } else {
                return false; // Cached as non-existent
            }
        }
    }

    // Save the cleaned key so we can cache the result later
    let cache_key = scratch.clone();

    // 3. Determine if it starts with "textures/" case-insensitively
    let starts_with_textures = scratch.len() >= 9 && scratch[..9].eq_ignore_ascii_case("textures/");

    let base = if starts_with_textures {
        let mut s = String::with_capacity(scratch.len());
        s.push_str("Textures/");
        s.push_str(&scratch[9..]);
        s
    } else {
        let mut s = String::with_capacity(9 + scratch.len());
        s.push_str("Textures/");
        s.push_str(scratch.as_str());
        s
    };

    let found = if resolve_manifest_path(&base, scratch) {
        true
    } else {
        resolve_bevy_asset_path(&base, scratch)
    };

    // 4. Store the result in the global cache
    if let Ok(mut cache) = path_cache().lock() {
        let entry = if found { Some(scratch.clone()) } else { None };
        cache.insert(cache_key, entry);
    }

    found
}

pub fn resolve_nif_model_path(model_path: &str, scratch: &mut String) -> bool {
    // 1. Clean path into scratch to use as our lookup key
    clean_path_common(model_path, scratch);
    if scratch.is_empty() {
        return false;
    }

    // 2. Check global runtime cache using the cleaned scratch string as a key
    if let Ok(cache) = path_cache().lock() {
        if let Some(cached_opt) = cache.get(scratch.as_str()) {
            if let Some(cached_path) = cached_opt {
                scratch.clear();
                scratch.push_str(cached_path);
                return true;
            } else {
                return false; // Cached as non-existent
            }
        }
    }

    // Save the cleaned key so we can cache the result later
    let cache_key = scratch.clone();

    // 3. Build the "data/Data Files/Meshes/..." base path
    let mut base = String::with_capacity(25 + scratch.len());
    base.push_str("data/Data Files/Meshes/");
    base.push_str(scratch.as_str());

    let found = if resolve_manifest_path(&base, scratch) {
        true
    } else {
        resolve_bevy_asset_path(&base, scratch)
    };

    // 4. Store the result in the global cache
    if let Ok(mut cache) = path_cache().lock() {
        let entry = if found { Some(scratch.clone()) } else { None };
        cache.insert(cache_key, entry);
    }

    found
}

pub fn resolve_bevy_asset_path(input_path: &str, scratch: &mut String) -> bool {
    resolve_manifest_path(input_path, scratch)
}

/// Used to turn names to u64 so it's smaller to store
pub const fn hash_str(s: &str) -> u64 {
    let bytes = s.as_bytes();
    let mut hash: u64 = 0xcbf29ce484222325; // FNV-1a offset basis
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(0x100000001b3); // FNV-1a prime
        i += 1;
    }
    hash
}
