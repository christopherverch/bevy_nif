#import bevy_pbr::mesh_view_bindings // For view.world_position, view.clip_from_world, view.projection
#import bevy_pbr::mesh_bindings    // For mesh.world_from_local (model matrix)
#import bevy_render::maths
#import bevy_pbr::pbr_types

// Import Bevy's PBR structures and functions
// The #define_import_path bevy_pbr::pbr_types you provided indicates where these structs come from.
#import bevy_pbr::pbr_types::{StandardMaterial, PbrInput, STANDARD_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE} // Added flags
#import bevy_pbr::pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing}
#import bevy_pbr::forward_io::{FragmentOutput, VertexOutput as PbrVertexOutput}


// Custom material bindings (Group 2) - Your existing struct
#import bevy_pbr::{
    mesh_bindings::mesh,
        mesh_functions,
        skinning,
        morph::morph,
        forward_io::{Vertex, VertexOutput},
        view_transformations::position_world_to_clip,
}
struct TerrainMaterialParams {
num_textures_in_array: u32,
                           diffuse_tiling_factor: f32,
};
@group(2) @binding(0) var source_textures_array: texture_2d_array<f32>;
@group(2) @binding(1) var tex_sampler: sampler;
@group(2) @binding(2) var alpha_maps: texture_2d_array<f32>;
@group(2) @binding(3) var alpha_map_sampler: sampler;
@group(2) @binding(4) var<uniform> material: TerrainMaterialParams;
#ifdef MORPH_TARGETS
fn morph_vertex(vertex_in: Vertex) -> Vertex {
    var vertex = vertex_in;
    let first_vertex = mesh[vertex.instance_index].first_vertex_index;
    let vertex_index = vertex.index - first_vertex;

    let weight_count = bevy_pbr::morph::layer_count();
    for (var i: u32 = 0u; i < weight_count; i ++) {
        let weight = bevy_pbr::morph::weight_at(i);
        if weight == 0.0 {
            continue;
        }
        vertex.position += weight * morph(vertex_index, bevy_pbr::morph::position_offset, i);
#ifdef VERTEX_NORMALS
        vertex.normal += weight * morph(vertex_index, bevy_pbr::morph::normal_offset, i);
#endif
#ifdef VERTEX_TANGENTS
        vertex.tangent += vec4(weight * morph(vertex_index, bevy_pbr::morph::tangent_offset, i), 0.0);
#endif
    }
    return vertex;
}
#endif
struct VertexInput {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv0: vec2<f32>,
    @location(5) vertex_color: vec4<f32>,
};

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let mesh_world_from_local = mesh_functions::get_world_from_local(in.instance_index);
    var world_from_local = mesh_world_from_local;
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
            in.normal,
            in.instance_index
            );

    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(in.position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
    out.uv = in.uv0;
    out.color = in.vertex_color;



#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
            in.instance_index, mesh_world_from_local[3]);
#endif

    return out;
}

@fragment
fn fragment(
        mesh: VertexOutput,
        ) -> @location(0) vec4<f32> {
       // --- 1. Calculate final albedo for the terrain (your existing logic) ---
    let tiled_uv = mesh.uv * material.diffuse_tiling_factor;
    var blended_diffuse_color_rgb = vec3<f32>(0.0);
    var total_weight_contribution = 0.0;
    let layers_to_sample = material.num_textures_in_array;

    if layers_to_sample == 0u {
        blended_diffuse_color_rgb = vec3<f32>(0.18); 
    } else {
        for (var i: u32 = 0u; i < layers_to_sample; i = i + 1u) {
            let diffuse_color_sample = textureSample(source_textures_array, tex_sampler, tiled_uv, i);
            let alpha_weight = textureSample(alpha_maps, alpha_map_sampler, mesh.uv, i).r;
            blended_diffuse_color_rgb += diffuse_color_sample.rgb * alpha_weight;
            total_weight_contribution += alpha_weight;
        }
        if total_weight_contribution > 0.001 {
            blended_diffuse_color_rgb /= total_weight_contribution;
        } else {
            if layers_to_sample > 0u {
                 blended_diffuse_color_rgb = textureSample(source_textures_array, tex_sampler, tiled_uv, 0u).rgb;
            } else {
                blended_diffuse_color_rgb = vec3<f32>(0.18); 
            }
        }
    }
   let albedo_linear_rgb = blended_diffuse_color_rgb * mesh.color.rgb;

    // --- 2. Construct PbrInput using the provided struct definitions ---
    var pbr_input: pbr_types::PbrInput;
     // Populate pbr_input.material (of type StandardMaterial)
    pbr_input.material.base_color = vec4<f32>(albedo_linear_rgb, 1.0);
    pbr_input.material.emissive = vec4<f32>(0.0);
    pbr_input.material.metallic = 0.00; // Very low for non-metallic
    pbr_input.material.perceptual_roughness = 0.95; // Quite rough for diffuse terrain
    pbr_input.material.reflectance = vec3<f32>(0.5); // Bevy's default, PBR functions will interpret this
                                                  // (often maps to ~0.04 linear F0 for dielectrics)
    pbr_input.material.flags = STANDARD_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE; // Basic opaque material
    pbr_input.material.uv_transform = mat3x3<f32>( // Identity UV transform at PBR material level
        vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0));
    // Default other StandardMaterial fields as per standard_material_new() or to "neutral" values
    pbr_input.material.attenuation_color = vec4<f32>(1.0);
    pbr_input.material.diffuse_transmission = 0.0;
    pbr_input.material.specular_transmission = 0.0;
    pbr_input.material.thickness = 0.0;
    pbr_input.material.ior = 1.2;
    pbr_input.material.attenuation_distance = 1.0; // Default might be large if thickness=0
    pbr_input.material.clearcoat = 0.0;
    pbr_input.material.clearcoat_perceptual_roughness = 0.0;
    pbr_input.material.anisotropy_strength = 0.0;
    pbr_input.material.anisotropy_rotation = vec2<f32>(0.0);
    pbr_input.material.alpha_cutoff = 0.5; // Not used for opaque
    pbr_input.material.parallax_depth_scale = 0.0; // No parallax in this setup
    pbr_input.material.max_parallax_layer_count = 0.0; // ""
    pbr_input.material.lightmap_exposure = 1.0;
    pbr_input.material.max_relief_mapping_search_steps = 0u;
    pbr_input.material.deferred_lighting_pass_id = 1u; // Default from standard_material_new

    // Populate other PbrInput fields
    pbr_input.frag_coord = mesh.position; // This is @builtin(position)
    pbr_input.world_position = vec4<f32>(mesh.world_position);
    pbr_input.world_normal = normalize(mesh.world_normal); // Use the geometric normal
    pbr_input.N = pbr_input.world_normal; // Shading normal = geometric normal (no material normal map here)
    pbr_input.V = normalize(mesh_view_bindings::view.world_position.xyz - mesh.world_position.xyz); // View vector

    pbr_input.diffuse_occlusion = vec3<f32>(1.0); // Default: no specific AO calculated here
    pbr_input.specular_occlusion = 1.0;   // Default: no specific AO
    pbr_input.lightmap_light = vec3<f32>(0.0);  // No lightmap
    
    // Clearcoat and Anisotropy fields default (as clearcoat=0, anisotropy_strength=0 on material)
    pbr_input.clearcoat_N = pbr_input.world_normal; 
    pbr_input.anisotropy_strength = pbr_input.material.anisotropy_strength; // from material
    pbr_input.anisotropy_T = vec3<f32>(1.0, 0.0, 0.0); // Dummy if strength is 0
    pbr_input.anisotropy_B = vec3<f32>(0.0, 1.0, 0.0); // Dummy if strength is 0

    // Determine if orthographic from view projection matrix
    // view.projection[3].w == 1.0 for orthographic, 0.0 for perspective.
    pbr_input.is_orthographic = (bevy_pbr::mesh_view_bindings::view.clip_from_view[3].w == 1.0);


    pbr_input.flags = 0u; // PbrInput specific flags, start with none.

    // --- 3. Apply Bevy's PBR lighting ---
    // This function uses the 'lights' and PBR 'view' uniforms bound by Bevy's PBR plugin
    var out_color_lit = apply_pbr_lighting(pbr_input);

    // --- 4. Apply Bevy's post-lighting processing (fog, tonemapping for non-HDR camera, etc.) ---
    out_color_lit = main_pass_post_lighting_processing(pbr_input, out_color_lit);

    return out_color_lit;

}
