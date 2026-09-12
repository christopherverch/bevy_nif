use bevy::prelude::*;
use esm_parser_lib::types::{creature::Creature, npc::Npc};

use crate::esm_ptr::EsmPtr;
#[derive(Component, Debug)]
pub struct LiveCellRef {
    pub base: CellRefBase,
}
#[derive(Clone, Debug)]
pub enum CellRefBase {
    Npc(EsmPtr<Npc>),
    Creature(EsmPtr<Creature>),
}
