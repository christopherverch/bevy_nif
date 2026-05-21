use bevy::prelude::*;
use esm_parser_lib::types::{creature::Creature, npc::Npc};
use std::ops::Deref;
#[derive(Component)]
pub enum LiveCellRef {
    Npc(EsmPtr<Npc>),
    Creature(EsmPtr<Creature>),
}

/// A wrapper to make raw pointers play nice with Bevy
pub struct EsmPtr<T>(pub *const T);

// SAFETY: This is safe because our EsmStore is a Global Resource
// that never moves or deletes the Boxed records.
unsafe impl<T> Send for EsmPtr<T> {}
unsafe impl<T> Sync for EsmPtr<T> {}

impl<T> Deref for EsmPtr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: Same as your existing get() logic—the pointer is
        // guaranteed to be valid for the lifetime of the App.
        unsafe { &*self.0 }
    }
}
