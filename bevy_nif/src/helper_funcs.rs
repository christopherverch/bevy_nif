use avian3d::parry::utils::hashmap::HashMap;
use std::sync::{Mutex, OnceLock};

#[cfg(target_arch = "wasm32")]
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
/// Wasm specific.
#[cfg(target_arch = "wasm32")]
pub fn check_exists(base: &str) -> bool {
    let normalized = base.replace('\\', "/");
    asset_manifest().contains(CiLookup::from_str(&normalized))
}

/// Resolves an asset path case-insensitively and writes the correct cased path to scratch
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
pub fn resolve_bevy_asset_path(input_path: &str, scratch: &mut String) -> bool {
    resolve_manifest_path(input_path, scratch)
}

#[cfg(not(target_arch = "wasm32"))]
fn get_asset_base_path() -> std::path::PathBuf {
    // Try CARGO_MANIFEST_DIR first (for `cargo run` development)
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let path = std::path::PathBuf::from(manifest_dir).join("assets");
        if path.exists() {
            return path;
        }
    }

    // Fallback to executable's directory (release builds)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let path = exe_dir.join("assets");
            if path.exists() {
                return path;
            }
        }
    }

    // Fallback to current working directory as a last resort
    std::path::PathBuf::from("assets")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn prepend_asset_path(str: &str) -> std::path::PathBuf {
    let mut path = get_asset_base_path();
    // Clean up leading slashes/backslashes
    let clean_str = str.trim_start_matches(['/', '\\']);
    path.push(clean_str);
    path
}
#[cfg(not(target_arch = "wasm32"))]
pub fn check_exists(base: &str) -> bool {
    let path = prepend_asset_path(base);
    path.exists()
}
#[cfg(not(target_arch = "wasm32"))]
pub fn resolve_bevy_asset_path(input_path: &str, scratch: &mut String) -> bool {
    use std::path::Path;

    let full_path = prepend_asset_path(input_path);
    let resolved_absolute = match resolve_case_insensitive_path(&full_path.to_string_lossy()) {
        Some(p) => p,
        None => return false,
    };

    let base_path = get_asset_base_path();
    let resolved_path = Path::new(&resolved_absolute);

    let rel_to_assets = match resolved_path.strip_prefix(&base_path) {
        Ok(p) => p,
        Err(_) => return false,
    };

    clean_path_common(&rel_to_assets.to_string_lossy(), scratch);
    !scratch.is_empty()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn resolve_case_insensitive_path(path: &str) -> Option<String> {
    use std::path::{Component, Path, PathBuf};

    let path_obj = Path::new(path);
    let mut current_path = PathBuf::new();

    for component in path_obj.components() {
        match component {
            Component::Prefix(prefix) => {
                current_path.push(prefix.as_os_str());
            }
            Component::RootDir => {
                current_path.push(component.as_os_str());
            }
            Component::CurDir => {
                if current_path.as_os_str().is_empty() {
                    current_path.push(".");
                }
            }
            Component::ParentDir => {
                if current_path.as_os_str().is_empty() {
                    current_path.push("..");
                } else {
                    current_path.pop();
                }
            }
            Component::Normal(s) => {
                let target_lower = s.to_string_lossy().to_lowercase();

                let dir_to_read = if current_path.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    current_path.as_path()
                };

                let entries = std::fs::read_dir(dir_to_read).ok()?;

                let matched = entries.flatten().find(|entry| {
                    entry.file_name().to_string_lossy().to_lowercase() == target_lower
                })?;

                current_path.push(matched.file_name());
            }
        }
    }

    current_path
        .is_file()
        .then(|| current_path.to_string_lossy().into_owned())
}
pub fn resolve_nif_texture_path(nif_path: &str, scratch: &mut String) -> bool {
    clean_path_common(nif_path, scratch);
    if scratch.is_empty() {
        return false;
    }

    if let Ok(cache) = path_cache().lock() {
        if let Some(cached_opt) = cache.get(scratch.as_str()) {
            if let Some(cached_path) = cached_opt {
                scratch.clear();
                scratch.push_str(cached_path);
                return true;
            } else {
                return false;
            }
        }
    }

    let cache_key = scratch.clone();
    let starts_with_textures = scratch.len() >= 9 && scratch[..9].eq_ignore_ascii_case("textures/");

    let base = if starts_with_textures {
        let mut s = String::with_capacity(scratch.len());
        s.push_str("Data Files/Textures/");
        s.push_str(&scratch[9..]);
        s
    } else {
        let mut s = String::with_capacity(9 + scratch.len());
        s.push_str("Data Files/Textures/");
        s.push_str(scratch.as_str());
        s
    };

    #[cfg(target_arch = "wasm32")]
    let found = if resolve_manifest_path(&base, scratch) {
        true
    } else {
        resolve_bevy_asset_path(&base, scratch)
    };

    #[cfg(not(target_arch = "wasm32"))]
    let found = if check_exists(&base) {
        scratch.clear();
        scratch.push_str(&base);
        true
    } else {
        resolve_bevy_asset_path(&base, scratch)
    };

    if let Ok(mut cache) = path_cache().lock() {
        let entry = if found { Some(scratch.clone()) } else { None };
        cache.insert(cache_key, entry);
    }

    found
}

pub fn resolve_nif_model_path(model_path: &str, scratch: &mut String) -> bool {
    clean_path_common(model_path, scratch);
    if scratch.is_empty() {
        return false;
    }

    if let Ok(cache) = path_cache().lock() {
        if let Some(cached_opt) = cache.get(scratch.as_str()) {
            if let Some(cached_path) = cached_opt {
                scratch.clear();
                scratch.push_str(cached_path);
                return true;
            } else {
                return false;
            }
        }
    }

    let cache_key = scratch.clone();
    let mut base = String::with_capacity(25 + scratch.len());
    base.push_str("Data Files/Meshes/");
    base.push_str(scratch.as_str());

    #[cfg(target_arch = "wasm32")]
    let found = if resolve_manifest_path(&base, scratch) {
        true
    } else {
        resolve_bevy_asset_path(&base, scratch)
    };

    #[cfg(not(target_arch = "wasm32"))]
    let found = if check_exists(&base) {
        scratch.clear();
        scratch.push_str(&base);
        true
    } else {
        resolve_bevy_asset_path(&base, scratch)
    };

    if let Ok(mut cache) = path_cache().lock() {
        let entry = if found { Some(scratch.clone()) } else { None };
        cache.insert(cache_key, entry);
    }

    found
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
