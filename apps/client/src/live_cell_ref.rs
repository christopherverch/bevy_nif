use bevy::prelude::*;
use esm_parser_lib::types::{creature::Creature, npc::Npc};

use crate::esm_ptr::EsmPtr;
#[derive(Component, Debug)]
pub struct LiveCellRef {
    pub base: CellRefBase,
}
#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub enum CellRefBase {
    Npc(EsmPtr<Npc>),
    Creature(EsmPtr<Creature>),
}
/// This macro gives us the EsmPtr without matching,
/// and just assumes the enum variant matches the type given
/// Debug builds check that the enum matches what we expect, doesn't check in release
macro_rules! impl_cell_ref_unwraps {
    ($($t:ident),* $(,)?) => {
        impl CellRefBase {
            $(
                #[inline(always)]
                pub unsafe fn extract_unchecked(&self) -> EsmPtr<$t> {
                    // Debug build actively checks the variant and panics if wrong
                    #[cfg(debug_assertions)]
                    {
                        if !matches!(self, CellRefBase::$t(_)) {
                            panic!(concat!("Type mismatch: expected ", stringify!($t)));
                        }
                    }

                  // Release build match + unreachable_unchecked tells the compiler to strip the check entirely
                  // and just read the pointer as whatever type was passed in
                    match self {
                        CellRefBase::$t(p) => *p,
                        _ => unsafe { core::hint::unreachable_unchecked() },
                    }
                }
            )*
        }
    };
}
impl_cell_ref_unwraps!(Npc);
