use esm_parser_lib::{Weapon, esm_record::EsmRecord, types::npc::Npc};
pub trait ActorRecord: EsmRecord + ClassTraits + Send + Sync + 'static {}
pub trait ClassTraits {
    fn base_can_fly(&self) -> bool {
        false // Default: everything is grounded
    }
    fn get_capacity(&self) -> f32 {
        panic!("get_capcity not implemented for this class type");
    }
}

///TODO::
pub fn get_capacity() -> f32 {
    1.0
}
///TODO::
pub fn get_encumbrance() -> f32 {
    1.0
}
pub fn get_normalized_encumbrance(capacity: f32, encumbrance: f32) -> f32 {
    // Intentional deviation: Morrowind doesn't do this
    // but handling (0 / 0) as 1.0 encumbrance feels like a clear oversight
    if encumbrance == 0.0 {
        return 0.0;
    }
    // Another deviation: handle (non-zero encumbrance / zero capacity) as "overencumbered"
    // Morrowind uses 1, but this means that for zero capacity,
    // normalized encumbrance cannot be used to detect overencumbrance
    if capacity == 0.0 {
        return 1.0 + 1e-6;
    }
    return encumbrance / capacity;
}

// Now these are empty impls!
impl ClassTraits for Weapon {}
impl ClassTraits for Npc {}

// Only the Creature needs the actual logic
// TODO::
/*
impl CanFly for Creature {
    fn base_can_fly(&self) -> bool {
        self.data.flags & 0x1 != 0
    }

    }



*/
