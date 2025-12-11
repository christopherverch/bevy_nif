use bevy::{
    asset::{AssetServer, Handle},
    color::{Color, LinearRgba},
    ecs::system::Res,
    image::Image,
    pbr::StandardMaterial,
    render::alpha::AlphaMode,
};
use nif::{
    NiMaterialProperty, NiTexturingProperty, NiType, TextureMap, TextureSource, loader::Nif,
};

use crate::helper_funcs::resolve_nif_path;

pub fn process_nitexturingproperty(
    tex_prop: &NiTexturingProperty,
    nif: &Nif,
    asset_server: &AssetServer,
) -> Option<Handle<Image>> {
    let mut texture_handle_opt: Option<Handle<Image>> = None;
    let base_texture = tex_prop.texture_maps[0].as_ref().unwrap();
    match base_texture {
        TextureMap::Map(tex_map) => {
            if let Some(tex_ni_type) = nif.objects.get(tex_map.texture.key) {
                match tex_ni_type {
                    NiType::NiSourceTexture(source_texture) => {
                        match &source_texture.source {
                            TextureSource::External(ext_path) => {
                                dbg!("loading");
                                dbg!(ext_path);
                                texture_handle_opt =
                                    Some(asset_server.load(resolve_nif_path(ext_path)));
                            }
                            TextureSource::Internal(link) => {
                                dbg!("Unimplemented!");
                                //TODO::
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        TextureMap::BumpMap(tex_bumpmap) => {
            dbg!("Unimplemented!");
        }
    }
    texture_handle_opt
}
pub fn process_nimaterialproperty(mat_prop: &NiMaterialProperty) -> StandardMaterial {
    let mut material = StandardMaterial::default();
    material.base_color = Color::srgb(
        mat_prop.diffuse_color[0],
        mat_prop.diffuse_color[1],
        mat_prop.diffuse_color[2],
    );
    material.emissive = LinearRgba::rgb(
        mat_prop.emissive_color[0],
        mat_prop.emissive_color[1],
        mat_prop.emissive_color[2],
    );
    material.metallic = 0.1;
    material.perceptual_roughness = 1.0 - (mat_prop.shine / 100.0).clamp(0.0, 1.0);
    material.alpha_mode = if mat_prop.alpha < 0.99 {
        AlphaMode::Blend
    } else {
        AlphaMode::Opaque
    };
    material
}
