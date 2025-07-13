use crate::nif::types::{NiTransform, NiTriShapeData};
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};
pub fn resolve_nif_path(nif_path: &str) -> String {
    // Basic cleanup - Needs proper implementation!
    let cleaned = nif_path.trim().replace('\\', "/");
    if !cleaned.starts_with("textures/") && !cleaned.is_empty() {
        format!("textures/{}", cleaned)
    } else {
        cleaned
    }
}
pub fn convert_nif_mesh(data: &NiTriShapeData) -> Option<Mesh> {
    // Check if essential data exists
    let vertices_nif = data.tri_base.geom_base.vertices.as_ref()?;
    let indices_nif = &data.triangles;

    if vertices_nif.is_empty() || indices_nif.is_empty() {
        return None; // No geometry
    }

    // Convert vertex positions
    let converted_vertices: Vec<[f32; 3]> = vertices_nif.iter().map(|v| [v.x, v.y, v.z]).collect();
    // Convert normals (if they exist)
    let converted_normals: Option<Vec<[f32; 3]>> = data
        .tri_base
        .geom_base
        .normals
        .as_ref()
        .map(|n| n.iter().map(|v| [v.x, v.y, v.z]).collect());

    // Convert UVs (use the first UV set if it exists)
    let converted_uvs: Option<Vec<[f32; 2]>> = data
        .tri_base
        .geom_base
        .uv_sets
        .get(0) // Get the first UV set
        .map(|uv_set| uv_set.iter().map(|v| [v.x, v.y]).collect());
    let final_mesh_opt: Option<Mesh>;

    // Create the Bevy Mesh
    if let Some(normals) = converted_normals {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, converted_vertices);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        if let Some(uvs) = converted_uvs {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        }

        mesh.insert_indices(Indices::U16(indices_nif.clone()));
        final_mesh_opt = Some(mesh);
    } else {
        final_mesh_opt = create_mesh_with_flat_normals(
            vertices_nif, // Pass the ORIGINAL NIF vertex slice reference (NOT converted_vertices)
            indices_nif,  // Pass slice reference to original indices
            converted_uvs.as_ref(), // Pass Option<&Vec<[f32; 2]>> for UVs
                          // (The helper needs to handle potential UV mismatch too)
        );
        // Generate flat normals if missing (or handle differently)
    }

    // NIF uses u16 indices

    // TODO: Add vertex colors (Mesh::ATTRIBUTE_COLOR) if data.has_vertex_colors is true

    // Potential Coordinate System Conversion:
    // If NIF is Z-up, Left-handed and Bevy is Y-up, Right-handed,
    // you might need to swizzle/negate vertex positions, normals, and adjust transforms.
    if let Some(mesh) = final_mesh_opt {
        Some(mesh)
    } else {
        None
    }
}
// Helper to convert NIF transform to Bevy transform
pub fn convert_nif_transform(nif_transform: &NiTransform) -> Transform {
    // --- 1. Extract NIF data into Bevy types (in NIF's coordinate space) ---

    // NIF Rotation Matrix to Bevy Quaternion
    // This assumes your Mat3::from_cols construction correctly interprets the NIF matrix layout.
    // It constructs a Bevy Mat3 using columns derived from the NIF data.
    let rot_mat = Mat3::from_cols(
        Vec3::new(
            nif_transform.rotation.0[0][0],
            nif_transform.rotation.0[1][0],
            nif_transform.rotation.0[2][0],
        ), // First Column
        Vec3::new(
            nif_transform.rotation.0[0][1],
            nif_transform.rotation.0[1][1],
            nif_transform.rotation.0[2][1],
        ), // Second Column
        Vec3::new(
            nif_transform.rotation.0[0][2],
            nif_transform.rotation.0[1][2],
            nif_transform.rotation.0[2][2],
        ), // Third Column
    );
    let initial_rotation = Quat::from_mat3(&rot_mat);

    let initial_translation = nif_transform.translation;
    // Assuming uniform scale. If NIF has non-uniform scale, it would need handling here too.
    let initial_scale = Vec3::splat(nif_transform.scale);

    // This transform represents the object in NIF's coordinate system (e.g., Z-up)
    let nif_space_transform = Transform {
        translation: initial_translation,
        rotation: initial_rotation,
        scale: initial_scale,
    };

    // --- 2. Define the Coordinate System Correction ---
    // We need to rotate the NIF coordinate system (assumed Z-up, Y-forward)
    // into Bevy's coordinate system (Y-up, -Z forward).
    // This requires a -90 degree rotation around the X axis.

    // --- 3. Apply the Correction ---
    // Multiply the correction by the original transform. This transforms the object
    // from NIF space into Bevy space.
    let bevy_space_transform = nif_space_transform;

    // --- 4. Return the corrected transform ---
    bevy_space_transform
}
fn create_mesh_with_flat_normals(
    original_vertices_nif: &Vec<Vec3>,
    original_indices: &[u16],
    original_uvs: Option<&Vec<[f32; 2]>>, // Pass converted UVs if available
) -> Option<Mesh> {
    let vertex_count = original_vertices_nif.len();
    if vertex_count == 0 {
        warn!("Cannot compute flat normals: No vertices provided.");
        return None;
    }
    if original_indices.is_empty() {
        warn!("Cannot compute flat normals: No indices provided.");
        return None;
    }
    if original_indices.len() % 3 != 0 {
        warn!(
            "Cannot compute flat normals: Index count ({}) is not a multiple of 3.",
            original_indices.len()
        );
        return None;
    }

    let num_triangles = original_indices.len() / 3;
    let new_vertex_count = num_triangles * 3;

    // Initialize buffers for the new mesh data
    let mut final_vertices: Vec<[f32; 3]> = Vec::with_capacity(new_vertex_count);
    let mut final_normals: Vec<[f32; 3]> = Vec::with_capacity(new_vertex_count);
    let mut final_indices: Vec<u16> = Vec::with_capacity(new_vertex_count);
    // Only create UV buffer if original UVs were present
    let mut final_uvs: Option<Vec<[f32; 2]>> =
        original_uvs.map(|_| Vec::with_capacity(new_vertex_count));

    for i in 0..num_triangles {
        // Get original vertex indices for this triangle
        let idx0_u16 = original_indices[i * 3];
        let idx1_u16 = original_indices[i * 3 + 1];
        let idx2_u16 = original_indices[i * 3 + 2];

        let idx0 = idx0_u16 as usize;
        let idx1 = idx1_u16 as usize;
        let idx2 = idx2_u16 as usize;

        // --- Bounds Checking ---
        if idx0 >= vertex_count || idx1 >= vertex_count || idx2 >= vertex_count {
            warn!(
                "Skipping triangle {} due to out-of-bounds index (Indices: {}, {}, {}; Vertex Count: {}).",
                i, idx0_u16, idx1_u16, idx2_u16, vertex_count
            );
            // To keep subsequent indices correct, we should perhaps push *something*
            // or adjust the final index buffer logic. For simplicity now, we skip.
            // This *could* lead to issues if not handled carefully later.
            // A safer approach might be to push degenerate data or filter indices beforehand.
            continue;
        }

        // Get vertex positions
        let v0 = original_vertices_nif[idx0];
        let v1 = original_vertices_nif[idx1];
        let v2 = original_vertices_nif[idx2];

        // Calculate face normal
        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        let face_normal = edge1.cross(edge2);

        // Normalize, handling degenerate triangles (use Y-up as default)
        let normalized_normal_array = face_normal.try_normalize().unwrap_or(Vec3::Y).to_array();

        // Add duplicated vertex positions
        final_vertices.push(v0.to_array());
        final_vertices.push(v1.to_array());
        final_vertices.push(v2.to_array());

        // Add the same face normal for all 3 vertices of this triangle
        final_normals.push(normalized_normal_array);
        final_normals.push(normalized_normal_array);
        final_normals.push(normalized_normal_array);

        // Add sequential indices for the new vertices
        let base_index = (i * 3) as u16; // Assuming less than 65536 triangles per original mesh part
        final_indices.push(base_index);
        final_indices.push(base_index + 1);
        final_indices.push(base_index + 2);

        // Duplicate UVs if they exist
        if let Some(ref mut uvs_out) = final_uvs {
            if let Some(uvs_in) = original_uvs {
                // Check bounds for original UVs as well
                if idx0 < uvs_in.len() && idx1 < uvs_in.len() && idx2 < uvs_in.len() {
                    uvs_out.push(uvs_in[idx0]);
                    uvs_out.push(uvs_in[idx1]);
                    uvs_out.push(uvs_in[idx2]);
                } else {
                    warn!(
                        "Missing UV data for original indices in triangle {}, using default [0,0].",
                        i
                    );
                    uvs_out.push([0.0, 0.0]);
                    uvs_out.push([0.0, 0.0]);
                    uvs_out.push([0.0, 0.0]);
                }
            }
        }
    }

    // --- Final Check ---
    if final_vertices.len() != final_normals.len() {
        error!(
            "Manual flat normal calculation resulted in mismatch! Verts: {}, Norms: {}",
            final_vertices.len(),
            final_normals.len()
        );
        // This shouldn't happen with this logic unless the continue above caused issues
        return None;
    }
    if let Some(ref uvs) = final_uvs {
        if final_vertices.len() != uvs.len() {
            error!(
                "Manual flat normal calculation resulted in UV mismatch! Verts: {}, UVs: {}",
                final_vertices.len(),
                uvs.len()
            );
            // Handle potential UV mismatch if necessary
        }
    }

    // Create the Bevy Mesh using the generated data
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, final_vertices) // Use new vertices
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, final_normals); // Use new normals

    if let Some(final_uvs_vec) = final_uvs {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, final_uvs_vec);
    }

    mesh.insert_indices(Indices::U16(final_indices)); // Use new sequential indices

    info!(
        "Successfully computed flat normals. Final vertex count: {}",
        mesh.count_vertices()
    );

    Some(mesh)
}
