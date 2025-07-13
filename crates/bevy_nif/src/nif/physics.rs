use bevy::prelude::{Asset, TypePath, Vec3};

// Import necessary types from your existing parser modules
use crate::{
    RecordLink,
    nif::{
        parser::helpers::find_node_by_name_recursive,
        types::{
            ParsedBlock, ParsedNifData,
            base::{BoundingBox, BoundingVolume},
            scene::NiNode,
        },
    },
};

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
        vertices: Vec<Vec3>,
        indices: Vec<[u32; 3]>,
    },
    Union(Vec<NifCollisionShape>),
}

/// The main entry point for collision extraction.
pub fn find_collision_shapes(nif_data: &ParsedNifData) -> Vec<NifCollisionShape> {
    nif_data
        .root_links
        .iter()
        .filter_map(|root_link| {
            let root_index = root_link.as_ref()?;
            let root_node = match nif_data.blocks.get(*root_index)? {
                ParsedBlock::Node(n) => n,
                _ => return None,
            };

            if has_no_collide_flag(&nif_data.blocks, root_node)
                || root_node.name().to_lowercase().contains("editormarker")
            {
                return None;
            }

            // --- EXCLUSIVE, PRIORITIZED SEARCH ---
            // This chain ensures that as soon as one method succeeds, the others are skipped.
            let shape = find_actor_bounding_box(&nif_data.blocks, *root_index)
                .or_else(|| find_static_collision_mesh(&nif_data.blocks, *root_index))
                .or_else(|| generate_mesh_from_all_geometry(&nif_data.blocks, root_node)); // This now calls the correct function below

            shape
        })
        .collect()
}

// --- Private Helper Functions ---

/// PRIORITY 1: Searches a hierarchy for a "Bounding Box" node.
fn find_actor_bounding_box(
    all_blocks: &[ParsedBlock],
    root_index: usize,
) -> Option<NifCollisionShape> {
    let bbox_node = find_node_by_name_recursive(all_blocks, root_index, "Bounding Box")?;
    let bv = bbox_node.bounding_volume.as_ref()?;
    Some(convert_bounding_volume_to_shape(bv))
}

/// PRIORITY 2: Searches a hierarchy for a "RootCollisionNode" and generates a mesh from its children.
fn find_static_collision_mesh(
    all_blocks: &[ParsedBlock],
    root_index: usize,
) -> Option<NifCollisionShape> {
    let collision_root_node =
        find_node_by_name_recursive(all_blocks, root_index, "RootCollisionNode")?;
    generate_mesh_from_node(all_blocks, collision_root_node)
}

/// PRIORITY 3 (FALLBACK): Generates a mesh from all geometry under a given node.
/// This is the function that was missing/mismatched before.
fn generate_mesh_from_all_geometry(
    all_blocks: &[ParsedBlock],
    start_node: &NiNode,
) -> Option<NifCollisionShape> {
    generate_mesh_from_node(all_blocks, start_node)
}

/// Helper function that actually builds a mesh shape from a starting node.
/// Used by both the "RootCollisionNode" path and the fallback path.
fn generate_mesh_from_node(
    all_blocks: &[ParsedBlock],
    start_node: &NiNode,
) -> Option<NifCollisionShape> {
    let mut all_vertices = Vec::new();
    let mut all_indices = Vec::new();

    collect_mesh_data_recursive(all_blocks, start_node, &mut all_vertices, &mut all_indices);

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

fn has_no_collide_flag(all_blocks: &[ParsedBlock], node: &NiNode) -> bool {
    let extra_data_indices = collect_all_extra_data_indices(all_blocks, node.extra_data_link);
    for extra_index in extra_data_indices {
        if let Some(ParsedBlock::StringExtraData(sed)) = all_blocks.get(extra_index) {
            if sed.string_data.to_uppercase().starts_with("NC") {
                return true;
            }
        }
    }
    false
}

fn collect_all_extra_data_indices(
    all_blocks: &[ParsedBlock],
    start_link: RecordLink,
) -> Vec<usize> {
    let mut chain = Vec::new();
    let mut current_link = start_link;
    while let Some(index) = current_link {
        chain.push(index);
        let Some(block) = all_blocks.get(index) else {
            break;
        };
        current_link = match block {
            ParsedBlock::StringExtraData(data) => data.extra_base.next_extra_data_link,
            ParsedBlock::TextKeyExtraData(data) => data.extra_base.next_extra_data_link,
            _ => break,
        };
    }
    chain
}

fn collect_mesh_data_recursive(
    all_blocks: &[ParsedBlock],
    current_node: &NiNode,
    all_vertices: &mut Vec<Vec3>,
    all_indices: &mut Vec<u32>,
) {
    for &child_link in &current_node.children {
        let Some(child_index) = child_link else {
            continue;
        };
        let Some(child_block) = all_blocks.get(child_index) else {
            continue;
        };

        match child_block {
            ParsedBlock::TriShape(trishape) => {
                if let Some(data_link) = trishape.data_link {
                    if let Some(ParsedBlock::TriShapeData(mesh_data)) = all_blocks.get(data_link) {
                        if let Some(verts) = &mesh_data.vertices {
                            let vertex_offset = all_vertices.len() as u32;
                            all_vertices.extend_from_slice(verts);
                            for index in &mesh_data.triangles {
                                all_indices.push(*index as u32 + vertex_offset);
                            }
                        }
                    }
                }
            }
            ParsedBlock::Node(node) => {
                collect_mesh_data_recursive(all_blocks, node, all_vertices, all_indices);
            }
            _ => {}
        }
    }
}
/// Converts the raw BoundingVolume from the NIF parser into a clean,
/// Bevy-coordinate-space-aware NifCollisionShape.
fn convert_bounding_volume_to_shape(bv: &BoundingVolume) -> NifCollisionShape {
    match bv {
        BoundingVolume::Box(BoundingBox { center, extent, .. }) => {
            // Apply coordinate swizzle to the center vector.
            let bevy_center = Vec3::new(-center.x, center.z, center.y);
            // Extents are absolute lengths, so we swizzle and take the absolute value.
            let bevy_extents = Vec3::new(extent.x.abs(), extent.z.abs(), extent.y.abs());

            NifCollisionShape::Box {
                center: bevy_center,
                extents: bevy_extents,
            }
        }
        BoundingVolume::Sphere(bs) => {
            // Apply coordinate swizzle to the center vector.
            let bevy_center = Vec3::new(-bs.center.x, -bs.center.z, bs.center.y);

            NifCollisionShape::Sphere {
                center: bevy_center,
                radius: bs.radius,
            }
        }
        BoundingVolume::Capsule(cap) => {
            // Apply coordinate swizzle to both the center and the axis direction vector.
            let bevy_center = Vec3::new(-cap.center.x, cap.center.z, cap.center.y);
            let bevy_axis = Vec3::new(-cap.axis.x, cap.axis.z, cap.axis.y);

            NifCollisionShape::Capsule {
                center: bevy_center,
                axis: bevy_axis,
                extent: cap.extent,
                radius: cap.radius,
            }
        }
        BoundingVolume::Union(children) => {
            // The recursive call handles the conversion for each child, so this works correctly.
            NifCollisionShape::Union(
                children
                    .iter()
                    .map(convert_bounding_volume_to_shape)
                    .collect(),
            )
        }
    }
}
