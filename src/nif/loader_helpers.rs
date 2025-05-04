fn convert_nif_mesh_base(data: &crate::nif::types::NiTriShapeData) -> Option<Mesh> {
    // Creates Mesh with positions, normals, UVs, BUT NOT skinning attributes yet
    // ... implementation needed ...
    None // Placeholder
}
fn convert_nif_material(mat_prop: &crate::nif::types::NiMaterialProperty) -> StandardMaterial {
    // Converts NiMaterialProperty to Bevy StandardMaterial (base properties)
    // ... implementation needed ...
    StandardMaterial::default() // Placeholder
}
fn convert_nif_translation_keys(keys: &[crate::nif::types::KeyVec3]) -> Vec<Keyframe<Translation>> {
    // Converts NIF translation KeyVec3 to Bevy Keyframes
    // ... implementation needed ...
    vec![] // Placeholder
}
fn convert_nif_rotation_keys(keys: &[crate::nif::types::KeyQuaternion]) -> Vec<Keyframe<Rotation>> {
    // Converts NIF KeyQuaternion to Bevy Keyframes
    // ... implementation needed ...
    vec![] // Placeholder
}
fn convert_nif_scale_keys(keys: &[crate::nif::types::KeyFloat]) -> Vec<Keyframe<Scale>> {
    // Converts NIF scale KeyFloat to Bevy Keyframes
    // ... implementation needed ...
    vec![] // Placeholder
}
