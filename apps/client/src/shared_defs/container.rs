use bevy::prelude::*;

use crate::shared_defs::item::Item;
#[derive(Component, Default)]
pub struct Container {
    pub weight_up_to_date: bool,
    pub cached_weight: f32,
    pub items: Vec<Item>,
}
impl Container {
    pub fn get_weight(&mut self) -> f32 {
        if !self.weight_up_to_date {
            self.cached_weight = 0.0;
            self.weight_up_to_date = true;
            for item in &self.items {
                self.cached_weight += item.encumbrance * item.count as f32;
            }
        }
        self.cached_weight
    }
}
