use avian3d::parry::utils::hashmap::HashMap;
use bevy_log::warn;
use std::path::{Component, Path};
use std::sync::{Mutex, OnceLock};
use std::{env, path::PathBuf};

fn cargo_manifest_dir() -> &'static str {
    static DIR: OnceLock<String> = OnceLock::new();
    DIR.get_or_init(|| {
        env::var("CARGO_MANIFEST_DIR").expect("Not running within a Cargo environment")
    })
}
fn path_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::default()))
}
pub fn prepend_asset_path(str: &str) -> PathBuf {
    let mut path = env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .expect("Not running within a Cargo environment");
    let mut prepended_asset_path = "assets/".to_string();
    prepended_asset_path.push_str(str);
    path.push(prepended_asset_path);
    path
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
pub fn check_exists(base: &str) -> bool {
    let path = prepend_asset_path(base);
    if path.exists() {
        return true;
    }
    false
}
pub fn resolve_nif_texture_path(nif_path: &str, scratch: &mut String) -> bool {
    // 1. Clean path into scratch to use as our lookup key
    clean_path_common(nif_path, scratch);
    if scratch.is_empty() {
        return false;
    }

    // 2. Check global cache using the cleaned scratch string as a key
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

    // Build the "Textures/..." base path
    // Since scratch is already populated, we format the base path.
    // (We can use a temporary local string or manipulate scratch carefully)
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

    let found = if check_exists(&base) {
        scratch.clear();
        scratch.push_str(&base);
        true
    } else {
        // Pass scratch down to resolve case-insensitively
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

    // 2. Check global cache using the cleaned scratch string as a key
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

    let found = if check_exists(&base) {
        scratch.clear();
        scratch.push_str(&base);
        true
    } else {
        // Fall back to case-insensitive Bevy asset path resolution
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
    let full_path = prepend_asset_path(input_path);
    let resolved_absolute = match resolve_case_insensitive_path(&full_path.to_string_lossy()) {
        Some(p) => p,
        None => return false,
    };

    let cargo_path = Path::new(cargo_manifest_dir());
    let resolved_path = Path::new(&resolved_absolute);

    let rel_to_cargo = match resolved_path.strip_prefix(cargo_path) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let rel_to_assets = match rel_to_cargo.strip_prefix("assets") {
        Ok(p) => p,
        Err(_) => return false,
    };

    clean_path_common(&rel_to_assets.to_string_lossy(), scratch);
    !scratch.is_empty()
}
/// Takes a path and tries to find a file with that name, case-insensitively, and if found returns
/// the actual filename
pub fn resolve_case_insensitive_path(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let path_obj = Path::new(&normalized);
    let mut current_path = PathBuf::new();

    for component in path_obj.components() {
        match component {
            Component::RootDir => {
                current_path.push("/");
            }
            Component::Prefix(prefix) => {
                current_path.push(prefix.as_os_str());
            }
            Component::Normal(s) => {
                let target_name = s.to_string_lossy();

                let dir_to_read = if current_path.as_os_str().is_empty() {
                    PathBuf::from(".")
                } else {
                    current_path.clone()
                };

                let entries = match std::fs::read_dir(&dir_to_read) {
                    Ok(e) => e,
                    Err(e) => {
                        warn!("failed to read dir: {}", e);
                        return None;
                    }
                };

                let mut found = false;
                let mut matched_name = PathBuf::new();

                for entry in entries.flatten() {
                    let file_name = entry.file_name();
                    if file_name
                        .to_string_lossy()
                        .eq_ignore_ascii_case(&target_name)
                    {
                        matched_name = file_name.into();
                        found = true;
                        break;
                    }
                }

                if !found {
                    return None;
                }

                current_path.push(matched_name);
            }
            Component::CurDir | Component::ParentDir => {}
        }
    }

    if current_path.is_file() {
        Some(current_path.to_string_lossy().into_owned())
    } else {
        None
    }
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
