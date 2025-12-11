use std::{io::Error, string::ParseError};

use bevy::{
    asset::{io::Reader, Asset, AssetLoader, LoadContext},
    log::error,
    reflect::TypePath,
};
use tempfile::{NamedTempFile, TempDir};


fn create_temp_file() -> (TempDir, NamedTempFile) {
    let dir = TempDir::new().unwrap();
    let file = NamedTempFile::new_in(&dir).unwrap();
    (dir, file)
}
