use bevy::color::Color;
use bevy::prelude::*;

mod generic_button;
mod race_select;
mod spell_description;
pub struct MWGuiPlugin;
impl Plugin for MWGuiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_spell_description_tooltip);
    }
}
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
// Renders from -z to +z, so lower on the list -> renders in front
pub enum RenderPosition {
    Background,
    Tooltip,
}
impl RenderPosition {
    pub fn as_f32(self) -> f32 {
        self as u8 as f32
    }
    pub fn as_i32(self) -> i32 {
        self as u8 as i32
    }
}
pub const UI_BACKGROUND: Color = Color::srgba(0.2, 0.2, 0.2, 0.5);
pub use race_select::setup_race_select;

use crate::gui::spell_description::setup_spell_description_tooltip;
