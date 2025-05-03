// src/nif/loader.rs

use crate::nif::error::ParseError; // Use path via nif module
use crate::nif::parser::parse_nif_start; // Use path via nif module
use crate::nif::structs::ParsedNifData; // Use path via nif module
use async_trait::async_trait;
use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*, // Use prelude for common types
};

#[derive(Default)]
pub struct NifAssetLoader; // Make struct public if main needs direct access (usually not)

#[async_trait]
impl AssetLoader for NifAssetLoader {
    type Asset = ParsedNifData;
    type Settings = ();
    type Error = ParseError;

    async fn load<'a>(
        &'a self,
        reader: &'a mut dyn Reader,
        _settings: &'a (),
        _load_context: &'a mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        parse_nif_start(&bytes)
    }

    fn extensions(&self) -> &[&str] {
        &["nif"]
    }
}
