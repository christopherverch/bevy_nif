use bevy::prelude::{Asset, TypePath, Vec3};
use slotmap::Key;

// Import necessary types from your existing parser modules

/// A generic, physics-engine-agnostic representation of a collision shape
/// extracted from a NIF file. This enum covers the shapes supported by OpenMW.
#[derive(Debug, Clone, Asset, TypePath)]
pub enum NifCollisionShape {
    Box {
        center: Vec3,
        extents: Vec3,
    },
    Sphere {
        center: Vec3,
        radius: f32,
    },
    Capsule {
        center: Vec3,
        axis: Vec3,
        extent: f32,
        radius: f32,
    },
    TriangleMesh {
        vertices: Vec<[f32; 3]>,
        indices: Vec<[u32; 3]>,
    },
    Union(Vec<NifCollisionShape>),
}
/*
/// The main entry point for collision extraction.
pub fn find_collision_shapes(stream: &NiStream) -> Vec<NifCollisionShape> {
    stream
        .roots // Iterate over the root links
        .iter()
        .filter_map(|root_link| {
            // 1. Get the root NiNode safely using the library's generic getter.
            let root_node: &NiNode = stream.get_as(*root_link)?;
            let root_key = root_link.key;

            // 2. Filter out editor markers and no-collide flags (assuming root_node.name exists)
            if has_no_collide_flag(stream, root_node)
                || root_node.name.to_lowercase().contains("editormarker")
            {
                return None;
            }

            // --- COLLISION PRIORITY CHAIN ---
            // 1. Dedicated Physics Block (Most Correct)
            let shape = find_static_collision_mesh(stream, root_key)
                // 2. Fallback to Node's Simple Bounding Box (Quick & Simple)
                .or_else(|| find_actor_bounding_box(stream, root_key))
                // 3. Fallback to Geometry Aggregation (Fallback, Slowest)
                .or_else(|| generate_mesh_from_all_geometry(stream, root_node));

            shape
        })
        .collect()
} // --- Private Helper Functions ---
*/
/// PRIORITY 1: Searches a hierarchy for a "Bounding Box" node.
fn find_actor_bounding_box(root_key: NiKey) -> Option<NifCollisionShape> {
    let bbox_node = find_node_by_name_recursive(stream, root_key, "Bounding Box")?;
    let bv = bbox_node.bounding_volume.as_ref()?;
    Some(convert_bounding_volume_to_shape(bv))
}

/// PRIORITY 2: Searches a hierarchy for a "RootCollisionNode" and generates a mesh from its children.
fn find_static_collision_mesh(stream: &NiStream, root_key: NiKey) -> Option<NifCollisionShape> {
    // 1. Find the dedicated collision node by name.
    let collision_root_node = find_node_by_name_recursive(stream, root_key, "RootCollisionNode")?;

    // 2. Generate the collision shape from all geometry under that node.
    // This is a placeholder for your actual geometry aggregation function.
    generate_mesh_from_node(stream, collision_root_node)
}
/// PRIORITY 3 (FALLBACK): Generates a mesh from all geometry under a given node.
/// This is the function that was missing/mismatched before.
fn generate_mesh_from_all_geometry(
    stream: &NiStream,
    start_node: &NiNode,
) -> Option<NifCollisionShape> {
    generate_mesh_from_node(stream, start_node)
}

/// Helper function that actually builds a mesh shape from a starting node.
/// Used by both the "RootCollisionNode" path and the fallback path.
fn generate_mesh_from_node(stream: &NiStream, start_node: &NiNode) -> Option<NifCollisionShape> {
    let mut all_vertices = Vec::new();
    let mut all_indices = Vec::new();

    collect_mesh_data_recursive(stream, start_node, &mut all_vertices, &mut all_indices);

    if all_indices.is_empty() {
        return None;
    }

    let tris: Vec<[u32; 3]> = all_indices
        .chunks_exact(3)
        .map(|chunk| [chunk[0], chunk[1], chunk[2]])
        .collect();

    Some(NifCollisionShape::TriangleMesh {
        vertices: all_vertices,
        indices: tris,
    })
}

// These helpers below are unchanged and correct.

fn has_no_collide_flag(stream: &NiStream, node: &NiNode) -> bool {
    let extra_data_indices = collect_all_extra_data_keys(stream, node.extra_data);
    for extra_key in extra_data_indices {
        if let Some(sed) = stream.get_as::<NiExtraData, NiStringExtraData>(NiLink::new(extra_key)) {
            if sed.value.to_uppercase().starts_with("NC") {
                return true;
            }
        }
    }
    false
}

fn collect_all_extra_data_keys(stream: &NiStream, start_link: NiLink<NiExtraData>) -> Vec<NiKey> {
    let mut chain = Vec::new();
    let mut current_key = start_link.key; // Start with the key from the link

    while !current_key.is_null() {
        chain.push(current_key);

        // 1. Try to get the block as NiStringExtraData
        let next_link = if let Some(sed) =
            stream.get_as::<NiExtraData, NiStringExtraData>(NiLink::new(current_key))
        {
            // The next link is contained within the block
            sed.base.next
        }
        // 2. If it wasn't a StringExtraData, try TextKeyExtraData
        else if let Some(tked) =
            stream.get_as::<NiExtraData, NiTextKeyExtraData>(NiLink::new(current_key))
        {
            // The next link is contained within the block
            tked.base.next
        }
        // 3. If it wasn't a recognized extra data type, we break the chain
        else {
            break;
        };

        // Move to the key of the next link
        current_key = next_link.key;
    }
    chain
}
fn collect_mesh_data_recursive(
    stream: &NiStream,
    current_node: &NiNode,
    all_vertices: &mut Vec<[f32; 3]>,
    all_indices: &mut Vec<u32>,
) {
    for child_link in &current_node.children {
        // 1. Check if the child is an NiNode (for recursion)
        if let Some(child_node) = stream.get_as::<NiAVObject, NiNode>(*child_link) {
            collect_mesh_data_recursive(stream, child_node, all_vertices, all_indices);
            continue;
        }

        // 2. Check if the child is a NiTriShape (a piece of geometry)
        if let Some(tri_shape) = stream.get_as::<NiAVObject, NiTriShape>(*child_link) {
            // The link is inside NiGeometry, which is the base of NiTriBasedGeom, which is the base of NiTriShape.
            let geometry_link = tri_shape.base.base.geometry_data;

            // 3. Get the linked NiGeometryData block
            // The link points to NiGeometryData, but the block is read as NiTriShapeData
            // We use the NiLink type found in the struct definition (NiGeometryData) to get the data block.
            // NOTE: Assuming NiTriShapeData also contains NiGeometryData fields.
            // If NiTriShapeData is the *actual* block type:
            // if let Some(tri_data) = stream.get_as::<NiGeometryData, NiTriShapeData>(geometry_link) {

            // We will use NiGeometryData as the target type for the required vertices field.
            if let Some(geom_data) = stream.get_as::<NiGeometryData, NiGeometryData>(geometry_link)
            {
                // 4. Collect vertices (FIXED: Access vertices directly on geom_data)
                let vertex_offset = all_vertices.len() as u32;

                all_vertices.extend(geom_data.vertices.iter().cloned());

                // 5. Collect and offset indices
                // We need the indices from the original NiTriShapeData struct, which is not fully defined here.
                // Assuming NiTriShapeData is accessible via the geometry_link and has the triangles field:

                // Re-fetch using NiTriShapeData type for indices
                if let Some(tri_data) =
                    stream.get_as::<NiGeometryData, NiTriShapeData>(geometry_link)
                {
                    for index in &tri_data.triangles {
                        all_indices.push((*index as u32) + vertex_offset);
                    }
                }
            }
        }
    }
}
/// Converts the raw BoundingVolume from the NIF parser into a
/// Bevy-coordinate-space-aware NifCollisionShape.
fn convert_bounding_volume_to_shape(bv: &NiBoundingVolume) -> NifCollisionShape {
    match &bv.bound_data {
        BoundData::NiBoxBV(box_bv) => {
            // Apply coordinate swizzle to the center vector.
            let bevy_center = Vec3::new(-box_bv.center.x, box_bv.center.z, box_bv.center.y);
            // Extents are absolute lengths, so we swizzle and take the absolute value.
            let bevy_extents = Vec3::new(
                box_bv.extents.x.abs(),
                box_bv.extents.z.abs(),
                box_bv.extents.y.abs(),
            );

            NifCollisionShape::Box {
                center: bevy_center,
                extents: bevy_extents,
            }
        }
        BoundData::NiSphereBV(sphere_bv) => {
            // Apply coordinate swizzle to the center vector.
            let bevy_center = Vec3::new(
                -sphere_bv.bound.center.x,
                -sphere_bv.bound.center.z,
                sphere_bv.bound.center.y,
            );

            NifCollisionShape::Sphere {
                center: bevy_center,
                radius: sphere_bv.bound.radius,
            }
        }
        BoundData::NiUnionBV(union_bv) => {
            // The recursive call handles the conversion for each child, so this works correctly.
            NifCollisionShape::Union(
                union_bv
                    .bounding_volumes
                    .iter()
                    .map(convert_bounding_volume_to_shape)
                    .collect(),
            )
        }
    }
}
