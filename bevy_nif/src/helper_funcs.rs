pub fn resolve_nif_path(nif_path: &str) -> String {
    let cleaned = nif_path.trim().replace('\\', "/");

    if cleaned.is_empty() {
        return cleaned;
    }

    // Case-insensitive check for "textures/"
    if cleaned.len() >= 9 && cleaned[..9].eq_ignore_ascii_case("textures/") {
        // Replace the prefix with proper casing
        format!("Textures/{}", &cleaned[9..])
    } else {
        // Prepend if missing
        format!("Textures/{}", cleaned)
    }
}
