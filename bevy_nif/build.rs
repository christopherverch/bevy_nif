use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::{env, fs};

fn main() {
    // Re-run build script if files in assets change or build.rs changes
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=../examples/wasm/assets");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("asset_manifest.rs");
    let temp_path = Path::new(&out_dir).join("asset_manifest.rs.tmp");
    let mut buffer = String::new();

    // 1. Locate the assets directory dynamically (relative to workspace or cargo manifest)
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let assets_path = Path::new(&manifest_dir)
        .join("../examples/wasm/assets")
        .canonicalize()
        .unwrap_or_else(|_| Path::new("..").join("examples/wasm/assets"));

    // 2. Recursively gather all asset paths
    let mut paths = Vec::new();
    if assets_path.exists() {
        collect_paths(&assets_path, &assets_path, &mut paths);
    }

    // 3. Write imports and custom case-insensitive types to generated code
    writeln!(buffer, "use std::collections::HashSet;").unwrap();
    writeln!(buffer, "use std::hash::{{Hash, Hasher}};").unwrap();
    writeln!(buffer, "use std::borrow::Borrow;").unwrap();
    writeln!(buffer, "use std::sync::OnceLock;").unwrap();
    writeln!(buffer, "").unwrap();

    // CiString definition
    writeln!(buffer, "#[derive(Debug, Eq)]").unwrap();
    writeln!(buffer, "pub struct CiString(&'static str);").unwrap();
    writeln!(buffer, "impl Hash for CiString {{").unwrap();
    writeln!(buffer, "    fn hash<H: Hasher>(&self, state: &mut H) {{").unwrap();
    writeln!(buffer, "        for b in self.0.as_bytes() {{").unwrap();
    writeln!(
        buffer,
        "            state.write_u8(b.to_ascii_lowercase());"
    )
    .unwrap();
    writeln!(buffer, "        }}").unwrap();
    writeln!(buffer, "    }}").unwrap();
    writeln!(buffer, "}}").unwrap();

    writeln!(buffer, "impl PartialEq for CiString {{").unwrap();
    writeln!(buffer, "    fn eq(&self, other: &Self) -> bool {{").unwrap();
    writeln!(buffer, "        self.0.eq_ignore_ascii_case(&other.0)").unwrap();
    writeln!(buffer, "    }}").unwrap();
    writeln!(buffer, "}}").unwrap();

    writeln!(buffer, "impl Borrow<str> for CiString {{").unwrap();
    writeln!(buffer, "    fn borrow(&self) -> &str {{").unwrap();
    writeln!(buffer, "        &self.0").unwrap();
    writeln!(buffer, "    }}").unwrap();
    writeln!(buffer, "}}").unwrap();

    // CiLookup definition
    writeln!(buffer, "#[repr(transparent)]").unwrap();
    writeln!(buffer, "pub struct CiLookup(str);").unwrap();
    writeln!(buffer, "impl CiLookup {{").unwrap();
    writeln!(buffer, "    pub fn from_str(s: &str) -> &Self {{").unwrap();
    writeln!(
        buffer,
        "        unsafe {{ &*(s as *const str as *const CiLookup) }}"
    )
    .unwrap();
    writeln!(buffer, "    }}").unwrap();
    writeln!(buffer, "}}").unwrap();

    writeln!(buffer, "impl Hash for CiLookup {{").unwrap();
    writeln!(buffer, "    fn hash<H: Hasher>(&self, state: &mut H) {{").unwrap();
    writeln!(buffer, "        for b in self.0.as_bytes() {{").unwrap();
    writeln!(
        buffer,
        "            state.write_u8(b.to_ascii_lowercase());"
    )
    .unwrap();
    writeln!(buffer, "        }}").unwrap();
    writeln!(buffer, "    }}").unwrap();
    writeln!(buffer, "}}").unwrap();

    writeln!(buffer, "impl PartialEq for CiLookup {{").unwrap();
    writeln!(buffer, "    fn eq(&self, other: &Self) -> bool {{").unwrap();
    writeln!(buffer, "        self.0.eq_ignore_ascii_case(&other.0)").unwrap();
    writeln!(buffer, "    }}").unwrap();
    writeln!(buffer, "}}").unwrap();
    writeln!(buffer, "impl Eq for CiLookup {{}}").unwrap();

    writeln!(buffer, "impl Borrow<CiLookup> for CiString {{").unwrap();
    writeln!(buffer, "    fn borrow(&self) -> &CiLookup {{").unwrap();
    writeln!(buffer, "        CiLookup::from_str(&self.0)").unwrap();
    writeln!(buffer, "    }}").unwrap();
    writeln!(buffer, "}}").unwrap();

    // 4. Write the paths as a flat constant array (compiles instantly!)
    writeln!(buffer, "pub const ASSET_PATHS: &[&str] = &[").unwrap();
    for path in &paths {
        let clean_path = path.replace('\\', "/");
        writeln!(buffer, "    \"{}\",", clean_path).unwrap();
    }
    writeln!(buffer, "];").unwrap();
    writeln!(buffer, "").unwrap();

    // Write the manifest function that initializes from the array
    writeln!(
        buffer,
        "pub fn asset_manifest() -> &'static HashSet<CiString> {{"
    )
    .unwrap();
    writeln!(
        buffer,
        "    static MANIFEST: OnceLock<HashSet<CiString>> = OnceLock::new();"
    )
    .unwrap();
    writeln!(buffer, "    MANIFEST.get_or_init(|| {{").unwrap();
    writeln!(
        buffer,
        "        let mut set = HashSet::with_capacity(ASSET_PATHS.len());"
    )
    .unwrap();
    writeln!(buffer, "        for &p in ASSET_PATHS {{").unwrap();
    writeln!(buffer, "            set.insert(CiString(p));").unwrap();
    writeln!(buffer, "        }}").unwrap();
    writeln!(buffer, "        set").unwrap();
    writeln!(buffer, "    }})").unwrap();
    writeln!(buffer, "}}").unwrap();

    // 5. Atomic write check (prevents unnecessary recompiles)
    let needs_update = if let Ok(existing) = fs::read_to_string(&dest_path) {
        existing != buffer
    } else {
        true
    };

    if needs_update {
        fs::write(&temp_path, buffer).expect("Failed to write temp manifest file");
        fs::rename(&temp_path, &dest_path).expect("Failed to rename manifest file");
    }
}

/// Recursively traverse folder and collect relative paths from assets root
fn collect_paths(root: &Path, current: &Path, paths: &mut Vec<String>) {
    let entries = match fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_paths(root, &path, paths);
        } else if path.is_file() {
            if let Ok(rel) = path.strip_prefix(root) {
                let rel_str = rel.to_string_lossy().to_string();
                if !rel_str.ends_with(".meta") {
                    paths.push(rel_str);
                }
            }
        }
    }
}
