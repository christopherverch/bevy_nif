// src/nif/mod.rs

// Declare the modules within this 'nif' module
pub mod error;
pub mod loader;
pub mod material; // Assuming ColorKeyExtension is in here or structs
pub mod mesh_conversion;
pub mod parser;
pub mod spawner;
pub mod structs;
pub mod transform_conversion;

// Re-export commonly used items from the submodules
// Adjust these based on what main.rs and other potential external crates might need
pub use error::ParseError;
pub use loader::NifAssetLoader;
pub use material::ColorKeyExtension; // Assuming it's defined here
pub use spawner::NifSceneRoot;
pub use structs::{NifHeader, NifTextureInfo, ParsedBlock, ParsedNifData}; // Add essential structs

