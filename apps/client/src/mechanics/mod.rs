mod character;
mod input;
mod sound;
mod weapontype;
use bevy::app::{App, Plugin};
pub use character::*;
pub use input::*;
pub use weapontype::*;

use crate::mechanics::sound::generate_sound;
mod const_defs;
pub struct MWMechanicsPlugin;
impl Plugin for MWMechanicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(generate_sound);
    }
}
