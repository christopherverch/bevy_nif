use bevy::{asset::RenderAssetUsages, prelude::*, render::mesh::PrimitiveTopology};
pub fn create_line_mesh(start: Vec3, end: Vec3) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::LineList, // Or LineStrip if you have connected lines
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![start.to_array(), end.to_array()],
    );
    // For LineList, each pair of vertices is a separate line.
    // If you were using LineStrip, it would connect them in order.
    mesh
}
