use std::{env, path::PathBuf};

pub fn resolve_nif_path(nif_path: &str) -> Option<String> {
    let cleaned = clean_path_common(nif_path);

    if cleaned.is_empty() {
        return None;
    }

    // Strip "textures/" if it exists and normalize to "Textures/"
    let base = if cleaned.len() >= 9 && cleaned[..9].eq_ignore_ascii_case("textures/") {
        format!("Textures/{}", &cleaned[9..])
    } else {
        format!("Textures/{}", cleaned)
    };
    // If it exists, just return it
    if check_exists(&base) {
        return Some(base);
    }

    // Otherwise, try and find the case-insensitive version of it
    resolve_bevy_asset_path(&base)
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
pub fn clean_path_common(path: &str) -> String {
    path.trim().replace('\\', "/").replace('\\', "")
}
pub fn check_exists(base: &str) -> bool {
    let path = prepend_asset_path(base);
    if path.exists() {
        return true;
    }
    false
}

use std::path::{Component, Path};

use bevy_log::warn;

/// Resolves a path case-insensitively inside the `assets/` folder
/// and returns the path relative to `assets/` for Bevy's asset server.
pub fn resolve_bevy_asset_path(input_path: &str) -> Option<String> {
    // Build the full absolute path starting from CARGO_MANIFEST_DIR/assets/
    let mut full_path = env::var("CARGO_MANIFEST_DIR").map(PathBuf::from).ok()?;

    // normalize weird windows filepaths
    let normalized_input = input_path.replace('\\', "/");
    let asset_relative = format!("assets/{}", normalized_input.trim_start_matches('/'));
    full_path.push(&asset_relative);

    // Resolve it case-insensitively using the absolute path
    let resolved_absolute = resolve_case_insensitive_path(&full_path.to_string_lossy())?;

    // Strip the absolute project and "assets/" prefix to get a relative path (i.e. music/song.mp3)
    // for bevy's asset loader
    let cargo_dir = env::var("CARGO_MANIFEST_DIR").ok()?;
    let cargo_path = Path::new(&cargo_dir);
    let resolved_path = Path::new(&resolved_absolute);

    let rel_to_cargo = resolved_path.strip_prefix(cargo_path).ok()?;
    let rel_to_assets = rel_to_cargo.strip_prefix("assets").ok()?;

    // Convert to a forward-slash string for Bevy
    let final_path = rel_to_assets
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_owned();

    Some(final_path)
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
