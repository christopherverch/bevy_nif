use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};
#[allow(dead_code)]
pub fn create_line_mesh(start: Vec3, end: Vec3) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default());
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![start.to_array(), end.to_array()],
    );
    mesh
}
