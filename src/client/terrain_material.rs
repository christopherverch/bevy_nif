use bevy::{
    prelude::*,
    reflect::TypePath,
    render::render_resource::{AsBindGroup, ShaderRef, ShaderType},
};

#[derive(Resource, Clone, Deref, DerefMut)]
pub struct TextureArrayHandle(pub Handle<Image>);
#[derive(Resource, Clone, Deref, DerefMut)]
pub struct AlphaMapTextureArrayHandle(pub Handle<Image>);

#[derive(ShaderType, Clone, Default, Debug)]
pub struct TerrainMaterialParams {
    pub num_textures_in_array: u32,
    pub diffuse_tiling_factor: f32,
    pub base_vtex_grid_dim: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TerrainBlendMaterial {
    #[texture(0, dimension = "2d_array")]
    #[sampler(1)]
    pub(crate) source_textures: Handle<Image>,
    #[texture(2, dimension = "2d_array")]
    #[sampler(3)]
    pub alpha_map_array: Handle<Image>,

    #[uniform(4)]
    pub(crate) params: TerrainMaterialParams,
}

impl Material for TerrainBlendMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain_blend_shader.wgsl".into()
    }

    fn vertex_shader() -> ShaderRef {
        "shaders/terrain_blend_shader.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}
