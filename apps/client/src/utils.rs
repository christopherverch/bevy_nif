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
pub fn prepend_mesh_data_path(path: String) -> String {
    let mut data_path = "data/Data Files/Meshes/".to_string();
    data_path.push_str(&path);
    // Standardize paths like abc.NIF to abc.nif
    if data_path.to_lowercase().ends_with(".nif") {
        let len = data_path.len();
        data_path.replace_range(len - 4.., ".nif");
    }
    data_path
}
