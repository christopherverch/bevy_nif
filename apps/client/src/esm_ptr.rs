use esm_store::EsmStore;
use std::ops::Deref;
/// A wrapper to make raw pointers work with Bevy
#[derive(Debug, Clone)]
pub struct EsmPtr<T>(pub *const T);

// SAFETY: EsmStore is a Global Resource, that after initial load can only be added to,
// and its backing data remains stable
unsafe impl<T> Send for EsmPtr<T> {}
unsafe impl<T> Sync for EsmPtr<T> {}

impl<T> Deref for EsmPtr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: the pointer is guaranteed to be valid for the lifetime of the reference
        unsafe { &*self.0 }
    }
}
impl<T> EsmPtr<T> {
    /// Safely yields a mutable reference
    /// Because it borrows _esm_store mutably nothing else can touch EsmStore until this reference drops.
    pub fn as_mut<'a>(&self, _esm_store: &'a mut EsmStore) -> &'a mut T {
        // SAFETY: We know the pointer originates from this store,
        // and taking &mut esm_store guarantees exclusive access.
        unsafe { &mut *(self.0 as *mut T) }
    }
}
