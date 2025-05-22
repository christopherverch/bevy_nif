// assets/shaders/terrain_prepass_vertex.wgsl

// Use the same namespacing for view and mesh that worked for your main shader
#import bevy_pbr::mesh_view_bindings
#import bevy_pbr::mesh_bindings
#import bevy_render::maths

// Vertex inputs should match your `specialize` function's layout
struct VertexInput {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    // @location(1) normal: vec3<f32>, 
    @location(2) uv0: vec2<f32>,
    @location(3) vertex_color: vec4<f32>, // ADDED: Input for vertex color
    // @location(4) tex_indices: vec4<i32>,  
};

struct PrepassVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(4) world_position: vec4<f32>,
    @location(5) previous_world_position: vec4<f32>,
    @location(7) @interpolate(flat) instance_index: u32,
    @location(8) color: vec4<f32>, // ADDED: Output for vertex color

    // If this prepass also requires normals, you would add them here, e.g.:
    // @location(2) world_normal: vec3<f32>; 
};

@vertex
fn vertex(in: VertexInput) -> PrepassVertexOutput {
    var out: PrepassVertexOutput;

    // Use the namespaced view and mesh variables
    let current_mesh_data = mesh_bindings::mesh[0]; 
    
    let model_matrix: mat4x4<f32> = bevy_render::maths::affine3_to_square(current_mesh_data.world_from_local); 
    let world_position_4 = model_matrix * vec4<f32>(in.position, 1.0);
    
    let previous_model_matrix: mat4x4<f32> = bevy_render::maths::affine3_to_square(current_mesh_data.previous_world_from_local);
    let previous_world_position_4 = previous_model_matrix * vec4<f32>(in.position, 1.0);

    out.clip_position = mesh_view_bindings::view.clip_from_world * world_position_4;
    out.uv = in.uv0;
    out.world_position = world_position_4;
    out.previous_world_position = previous_world_position_4;
    out.instance_index = in.instance_index;
    out.color = in.vertex_color; // ADDED: Pass vertex color through

    return out;
}
