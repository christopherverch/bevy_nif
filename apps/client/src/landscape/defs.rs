use bevy::prelude::*;
#[derive(Component, Debug, Copy, Clone)]
pub struct LandscapeCell {
    pub x: i32,
    pub y: i32,
}
#[derive(Component)]
pub struct LandscapeLoader {
    pub cell_x: i32,
    pub cell_y: i32,
}
