// In a new file (e.g., terrain_material.rs) or an existing module

use bevy::{
    pbr::{MaterialPipeline, MaterialPipelineKey}, // For Material
    prelude::*,
    reflect::TypePath,
    render::{
        mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef, VertexFormat},
        render_resource::{
            AsBindGroup, RenderPipelineDescriptor, ShaderRef, ShaderType,
            SpecializedMeshPipelineError,
        },
    },
};

// Your custom vertex attribute ID (ensure this is defined and accessible)
pub const ATTRIBUTE_TERRAIN_TEXTURE_INDICES: MeshVertexAttribute = MeshVertexAttribute::new(
    "Vertex_Terrain_Texture_Indices",
    99887766,
    VertexFormat::Sint32x4,
);

// The resource you created to hold the handle
#[derive(Resource, Clone, Deref, DerefMut)]
pub struct TextureArrayHandle(pub Handle<Image>);
#[derive(Resource, Clone, Deref, DerefMut)]
pub struct AlphaMapTextureArrayHandle(pub Handle<Image>); // For alpha maps

#[derive(ShaderType, Clone, Default, Debug)]
pub struct TerrainMaterialParams {
    pub num_textures_in_array: u32,
    pub diffuse_tiling_factor: f32,
    // Add any other parameters your shader might need
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TerrainBlendMaterial {
    // This handle MUST point to an Image asset that IS a Texture Array
    // Bevy's AsBindGroup will attempt to create a TextureView from this.
    // It's crucial that the Image asset was created with depth_or_array_layers > 1
    // and TextureDimension::D2. Bevy should then create a TextureView with D2Array.
    #[texture(0, dimension = "2d_array")] // This tells the binding to expect a 2D array texture view
    #[sampler(1)]
    pub(crate) source_textures: Handle<Image>, // Changed name for clarity
    #[texture(2, dimension = "2d_array")] // New binding for the alpha map array
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
        AlphaMode::Opaque // Or AlphaMode::Blend if needed
    }
    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/terrain_prepass_vertex.wgsl".into()
    }
    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(3), // VCLR data
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        Ok(())
    }
}
