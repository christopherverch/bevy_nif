// Internal
use crate::defs::GameState;
use crate::gui::race_select::SliderDragStart;
use crate::gui::server_selection::{
    FocusedIpInput, button_system, despawn_main_menu_ui, listen_keyboard_input_events,
    set_initial_focus, setup_main_menu,
};
use crate::gui::spell_description::setup_spell_description_tooltip;
pub use race_select::setup_race_select;

mod generic_button;
mod race_select;
mod server_selection;
mod spell_description;

// External
use bevy::prelude::*;

pub struct MWGuiPlugin;
impl Plugin for MWGuiPlugin {
    fn build(&self, app: &mut App) {
        // Initial server selection
        app.insert_resource(FocusedIpInput::default())
            .add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
            .add_systems(OnEnter(GameState::MainMenu), set_initial_focus)
            .add_systems(OnEnter(GameState::Connecting), despawn_main_menu_ui)
            .add_systems(Update, button_system.run_if(in_state(GameState::MainMenu)))
            .add_systems(
                Update,
                listen_keyboard_input_events.run_if(in_state(GameState::MainMenu)),
            );

        // Race selection
        app.insert_resource(SliderDragStart::default())
            .add_systems(Startup, setup_spell_description_tooltip);
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
