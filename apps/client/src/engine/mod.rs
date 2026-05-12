use bevy::prelude::*;
mod main_loop;
use main_loop::main_loop;
pub struct EnginePlugin;
impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, main_loop);
    }
}
