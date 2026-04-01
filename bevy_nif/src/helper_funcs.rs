use std::{env, path::PathBuf};

pub fn resolve_nif_path(nif_path: &str) -> Option<String> {
    let cleaned = nif_path.trim().replace('\\', "/");

    if cleaned.is_empty() {
        return None;
    }

    let base = if cleaned.len() >= 9 && cleaned[..9].eq_ignore_ascii_case("textures/") {
        format!("Textures/{}", &cleaned[9..])
    } else {
        format!("Textures/{}", cleaned)
    };

    let mut path = env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .expect("Not running within a Cargo environment");
    let mut prepended_asset_path = "assets/".to_string();
    prepended_asset_path.push_str(&base);
    path.push(prepended_asset_path);
    // 1. Try original

    if path.exists() {
        return Some(base);
    }

    // 2. Try lowercase extension
    if let Some(dot_index) = base.rfind('.') {
        let (name, ext) = base.split_at(dot_index);
        let lower = format!("{}{}", name, ext.to_lowercase());
        let mut path = env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .expect("Not running within a Cargo environment");
        let mut prepended_asset_path = "assets/".to_string();
        prepended_asset_path.push_str(&lower);
        path.push(prepended_asset_path);

        if path.exists() {
            return Some(lower);
        }
    }

    None
}
