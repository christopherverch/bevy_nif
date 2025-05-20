// assets/shaders/terrain_blend_shader.wgsl
#import bevy_pbr::mesh_view_bindings::View 
#import bevy_pbr::mesh_functions::Mesh
// Bevy's view and mesh uniforms
@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var<uniform> mesh: Mesh;

// Custom material bindings (must match AsBindGroup in Rust)
struct TerrainMaterialParams {
    tiling_factor: f32,
    num_textures_in_array: u32,
    // Add other params if needed
    // Bevy might add padding here for alignment, so ensure Rust struct matches
    // For example, if num_textures_in_array is the last element,
    // you might need vec2<f32> or similar padding if total size isn't multiple of 16 bytes.
    // Or ensure Rust struct is also padded if necessary for a specific WGSL layout.
    // For now, assuming simple case.
};

@group(2) @binding(0) var source_textures_array: texture_2d_array<f32>;
@group(2) @binding(1) var tex_sampler: sampler;
@group(2) @binding(2) var<uniform> material: TerrainMaterialParams;


struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) vertex_color: vec4<f32>, // VCLR (r,g,b are weights, a is likely 1.0)
    @location(4) tex_indices: vec4<i32>,  // Slot indices into texture_array (Sint32x4)
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv0: vec2<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) vertex_color_weights: vec4<f32>,
    @location(3) texture_array_indices: vec4<i32>,
    // @location(4) world_position: vec3<f32>, // Optional for more advanced lighting
};

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_position_4 = mesh.model * vec4<f32>(in.position, 1.0);
    out.clip_position = view.view_proj * world_position_4;
    out.uv0 = in.uv0;
    out.world_normal = normalize((mesh.model * vec4<f32>(in.normal, 0.0)).xyz);
    out.vertex_color_weights = in.vertex_color; // Pass VCLR weights (r,g,b are primary)
    out.texture_array_indices = in.tex_indices;
    // out.world_position = world_position_4.xyz;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var blended_color: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    let tiled_uv = in.uv0 * material.tiling_factor;

    // Blend weights from VCLR (assuming .rgb are the main weights)
    let weight_r = in.vertex_color_weights.r;
    let weight_g = in.vertex_color_weights.g;
    let weight_b = in.vertex_color_weights.b;
    // The 4th weight is for the "base" texture, what's left over.
    // Ensure sum_rgb_weights doesn't exceed 1.0 for sensible results.
    let sum_rgb_weights = clamp(weight_r + weight_g + weight_b, 0.0, 1.0);
    let weight_base = 1.0 - sum_rgb_weights;

    // Texture for Red channel weight (slot 0 in tex_indices)
    if (in.texture_array_indices.x >= 0 && weight_r > 0.001) { // Check for valid index & weight
        blended_color += textureSample(source_textures_array, tex_sampler, tiled_uv, u32(in.texture_array_indices.x)) * weight_r;
    }
    // Texture for Green channel weight (slot 1 in tex_indices)
    if (in.texture_array_indices.y >= 0 && weight_g > 0.001) {
        blended_color += textureSample(source_textures_array, tex_sampler, tiled_uv, u32(in.texture_array_indices.y)) * weight_g;
    }
    // Texture for Blue channel weight (slot 2 in tex_indices)
    if (in.texture_array_indices.z >= 0 && weight_b > 0.001) {
        blended_color += textureSample(source_textures_array, tex_sampler, tiled_uv, u32(in.texture_array_indices.z)) * weight_b;
    }
    
    // Texture for the calculated base/remaining weight (slot 3 in tex_indices)
    if (in.texture_array_indices.w >= 0 && weight_base > 0.001) {
         blended_color += textureSample(source_textures_array, tex_sampler, tiled_uv, u32(in.texture_array_indices.w)) * weight_base;
    } else if (sum_rgb_weights < 0.001 && in.texture_array_indices.x >=0) { 
        // If no VCLR weights, default to the first specified texture (SW)
        // (or use index from .w if it's meant to be the absolute base)
         blended_color = textureSample(source_textures_array, tex_sampler, tiled_uv, u32(in.texture_array_indices.x));
    }


    blended_color.a = 1.0; // Ensure final alpha is opaque

    // Basic lighting
    let N = normalize(in.world_normal);
    let L = normalize(vec3<f32>(0.5, 1.0, 0.5)); // Example light direction
    let diffuse_lighting = max(dot(N, L), 0.0);
    let ambient = 0.2;
    
    let final_rgb = blended_color.rgb * (diffuse_lighting * 0.8 + ambient);

    return vec4<f32>(final_rgb, blended_color.a);
}

