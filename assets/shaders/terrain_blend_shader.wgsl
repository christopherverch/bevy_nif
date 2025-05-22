#import bevy_pbr::mesh_view_bindings;
#import bevy_pbr::mesh_bindings; 
#import bevy_render::maths;

// NO NEED to declare 'view' (comes from mesh_view_bindings)
// NO NEED to declare 'mesh' (comes from mesh_bindings)

// Custom material bindings (Group 2)
struct TerrainMaterialParams {
num_textures_in_array: u32,
                           diffuse_tiling_factor: f32,

};

@group(2) @binding(0) var source_textures_array: texture_2d_array<f32>;
@group(2) @binding(1) var tex_sampler: sampler;
@group(2) @binding(2) var alpha_maps: texture_2d_array<f32>;
@group(2) @binding(3) var alpha_map_sampler: sampler;
@group(2) @binding(4) var<uniform> material: TerrainMaterialParams;


struct VertexInput {
    @location(0) position: vec3<f32>,
        @location(1) normal: vec3<f32>,
        @location(2) uv0: vec2<f32>,
        @location(3) vertex_color: vec4<f32>, 
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
        @location(0) uv0: vec2<f32>,
        @location(1) world_normal: vec3<f32>,
        @location(2) vertex_color_weights: vec4<f32>,
        @location(3) world_position: vec3<f32>,
};

@vertex
fn vertex(
        in: VertexInput,
        ) -> VertexOutput {
    var out: VertexOutput;
    let current_mesh_data = mesh_bindings::mesh[0]; // Assuming index 0 for the current mesh
    let model_matrix: mat4x4<f32> = bevy_render::maths::affine3_to_square(current_mesh_data.world_from_local); 
    let world_position_4 = model_matrix * vec4<f32>(in.position, 1.0);
    out.world_position = world_position_4.xyz;
    out.clip_position = mesh_view_bindings::view.clip_from_world * world_position_4; 
    out.uv0 = in.uv0;

    let world_normal_matrix = mat3x3<f32>(
            model_matrix[0].xyz,
            model_matrix[1].xyz,
            model_matrix[2].xyz
            );
    out.world_normal = normalize(world_normal_matrix * in.normal);
    out.vertex_color_weights = in.vertex_color;
    return out;
}

// Fragment shader remains the same
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tiled_uv = in.uv0 * material.diffuse_tiling_factor;
    
    var blended_diffuse_color_rgb = vec3<f32>(0.0);
    var total_weight_contribution = 0.0;

    // Determine number of layers to actually process, respecting a shader-defined max if any
    // const MAX_SHADER_LAYERS = 6u; // Example, define globally in WGSL if needed
    // let layers_to_sample = min(material.num_textures_in_array, MAX_SHADER_LAYERS);
    let layers_to_sample = material.num_textures_in_array; // Or directly use, if material ensures it's not too large

    if layers_to_sample == 0u { // Handle case with no textures
        return vec4<f32>(0.1, 0.1, 0.1, 1.0); // Default dark color
    }

    // First pass: gather colors and weights
    // If using fixed-size arrays in WGSL, ensure they are large enough (e.g., MAX_SHADER_LAYERS)
    // For simplicity here, let's assume direct accumulation if possible, or use small fixed arrays.
    // Direct accumulation (if alpha maps are masks ensuring one dominant layer or sum to <=1)
    for (var i: u32 = 0u; i < layers_to_sample; i = i + 1u) {
        let diffuse_color_sample = textureSample(source_textures_array, tex_sampler, tiled_uv, i);
        let alpha_weight = textureSample(alpha_maps, alpha_map_sampler, in.uv0, i).r; // Assuming R8Unorm

        blended_diffuse_color_rgb += diffuse_color_sample.rgb * alpha_weight;
        total_weight_contribution += alpha_weight;
    }

    // Normalize if weights don't inherently sum to 1 (e.g. due to smoothed alpha map edges)
    if total_weight_contribution > 0.001 {
        blended_diffuse_color_rgb /= total_weight_contribution;
    } else {
        // Fallback if no alpha contribution, e.g., use first texture or default color
        if layers_to_sample > 0u {
             blended_diffuse_color_rgb = textureSample(source_textures_array, tex_sampler, tiled_uv, 0u).rgb;
        } else {
            blended_diffuse_color_rgb = vec3<f32>(0.1, 0.1, 0.1); // Default if truly no layers
        }
    }

    // Apply VCLR vertex color tint
    let tinted_rgb = blended_diffuse_color_rgb * in.vertex_color_weights.rgb;

    // Simple directional lighting
    let N = normalize(in.world_normal);
    let L = normalize(vec3<f32>(0.577, 0.577, 0.577)); // Example light direction
    let ambient_color = vec3<f32>(0.53725, 0.5490, 0.62745098); 
    
    let diffuse_intensity = max(dot(N, L), 0.0);
    let final_lit_rgb = tinted_rgb * diffuse_intensity + tinted_rgb * ambient_color;

    return vec4<f32>(final_lit_rgb, 1.0);
}
