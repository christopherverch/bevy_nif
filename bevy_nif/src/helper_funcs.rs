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
    if check_exists(&base) {
        return Some(base);
    }

    // 2. Try lowercase extension
    return check_exists_lowercase_extension(&base);
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
    path.trim().replace('\\', "/")
}
pub fn check_exists(base: &str) -> bool {
    let path = prepend_asset_path(base);
    //dbg!(&path);
    if path.exists() {
        return true;
    }
    false
}
pub fn check_exists_lowercase_extension(path: &str) -> Option<String> {
    if let Some(dot_index) = path.rfind('.') {
        let (name, ext) = path.split_at(dot_index);
        let lower = format!("{}{}", name, ext.to_lowercase());
        let path = prepend_asset_path(&lower);
        if path.exists() {
            return Some(lower);
        }
    }
    None
}
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
