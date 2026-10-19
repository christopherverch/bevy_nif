pub mod loader;
pub mod types;
use bevy_math::{Mat3, Quat, Vec3};
pub use types::*;

pub(crate) mod macros;

#[allow(unused_imports)]
pub(crate) mod prelude {
    pub use super::*;

    // internal imports
    pub use bytes_io::*;
    pub use macros::*;
    pub use nif_macros::*;

    // external imports
    pub use bstr::{BString, ByteSlice, ByteVec};
    pub use bytemuck::{NoUninit, Pod, Zeroable};
    pub use derive_more::{Deref, DerefMut, From, Into};
    pub use hashbrown::{HashMap, HashSet};
    pub use smart_default::SmartDefault;
    pub use std::io;

    // use [`std::default::default`] when stable
    pub fn default<T: Default>() -> T {
        Default::default()
    }

    // basic math types
    mod math {
        pub use bevy_math::{Affine3A, Mat2, Mat3, Quat, Vec2, Vec3, Vec4};
        // a temporary alias until we pick a color library
        pub type ColorA = Vec4;
    }
    pub use math::*;
}
const NIF_SCALE_FACTOR: f32 = 69.991251091;
fn nif_to_bevy_vec3(v: Vec3) -> Vec3 {
    Vec3::new(
        -v.x, // Flip X for handedness conversion
        v.z,  // NIF Z (up) becomes Bevy Y (up)
        v.y,  // NIF Y (forward) becomes Bevy Z (depth)
    ) / NIF_SCALE_FACTOR
}
fn bevy_to_nif_vec3(v: Vec3) -> Vec3 {
    Vec3::new(-v.x, v.z, v.y) * NIF_SCALE_FACTOR
}
fn nif_to_bevy_normal(v: Vec3) -> Vec3 {
    Vec3::new(-v.x, v.z, v.y).normalize() // Keep them normalized unit vectors
}
fn nif_to_bevy_mat3(mat: Mat3) -> Mat3 {
    let m = Mat3::from_cols(
        Vec3::new(-1.0, 0.0, 0.0), // NIF X -> Bevy -X
        Vec3::new(0.0, 0.0, 1.0),  // NIF Y -> Bevy Z
        Vec3::new(0.0, 1.0, 0.0),  // NIF Z -> Bevy Y
    );
    (m * mat * m).inverse()
}
fn nif_to_bevy_quat(q: Quat) -> Quat {
    let m = Mat3::from_cols(
        Vec3::new(-1.0, 0.0, 0.0), // NIF X -> Bevy -X
        Vec3::new(0.0, 0.0, 1.0),  // NIF Y -> Bevy Z
        Vec3::new(0.0, 1.0, 0.0),  // NIF Z -> Bevy Y
    );
    let r_nif = Mat3::from_quat(q);
    let r_bevy = m * r_nif * m;

    Quat::from_mat3(&r_bevy.inverse())
}
