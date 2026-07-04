use std::{env, path::PathBuf};

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
pub fn prepend_asset_path(str: &str) -> PathBuf {
    let mut path = env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .expect("Not running within a Cargo environment");
    let mut prepended_asset_path = "assets/".to_string();
    prepended_asset_path.push_str(str);
    path.push(prepended_asset_path);
    path
}
