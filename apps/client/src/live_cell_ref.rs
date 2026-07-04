use bevy::prelude::*;
use esm_parser_lib::types::{creature::Creature, npc::Npc};
use esm_store::EsmStore;
use std::ops::Deref;
#[derive(Component, Debug)]
pub enum LiveCellRef {
    Npc(EsmPtr<Npc>),
    Creature(EsmPtr<Creature>),
}

/// A wrapper to make raw pointers work with Bevy
#[derive(Debug, Clone)]
pub struct EsmPtr<T>(pub *const T);

// SAFETY: This is safe because our EsmStore is a Global Resource
// that never moves or deletes the Boxed records.
unsafe impl<T> Send for EsmPtr<T> {}
unsafe impl<T> Sync for EsmPtr<T> {}

impl<T> Deref for EsmPtr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: the pointer is guaranteed to be valid for the lifetime of the reference
        unsafe { &*self.0 }
    }
}
/// Casts the esm ptr to a *mut T, takes in &mut EsmStore to make sure we have exclusive access to
/// it since esmptr points into it
pub fn esm_ptr_as_mut<T>(ptr: EsmPtr<T>, _esm_store: &mut EsmStore) -> *mut T {
    ptr.0 as *mut T
}
