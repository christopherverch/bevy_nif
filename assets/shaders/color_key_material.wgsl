#import bevy_pbr::forward_io::VertexOutput
#import "shaders/custom_material_import.wgsl"::COLOR_MULTIPLIER
// we can import items from shader modules in the assets folder with a quoted path
@group(2) @binding(0) var<uniform> material_color: vec4<f32>;
@group(2) @binding(1) var material_color_texture: texture_2d<f32>;
@group(2) @binding(2) var material_color_sampler: sampler;

@fragment
fn fragment(
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {
    // Calculate the original color first
    var final_color: vec4<f32> = material_color * textureSample(material_color_texture, material_color_sampler, mesh.uv);

    // --- Start Modification ---
    // Define a threshold for what counts as "black".
    // Using a small threshold is often better than == 0.0 due to floating point inaccuracies.
    // Adjust this value if needed (e.g., 0.001 for stricter black, 0.05 for near-black).
    let black_threshold: f32 = 0.01;

    // Check if the Red, Green, AND Blue components are all below the threshold
    if (final_color.r < black_threshold &&
        final_color.g < black_threshold &&
        final_color.b < black_threshold) {
        // If the color is considered black, set its alpha component to 0.0 (fully transparent)
    }
    // --- End Modification ---

    // Return the potentially modified color
    return final_color;
}
