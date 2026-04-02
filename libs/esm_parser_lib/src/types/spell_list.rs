use crate::{EsmError, EsmReader, RefId};

/// A list of references to spells and spell effects. This is shared
/// between the records BSGN, NPC and RACE.
/// NPCS subrecord.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpellList {
    pub list: Vec<RefId>,
}

impl SpellList {
    /// Loads a single spell NPCS subrecord and adds it to the list.
    /// assumes the subrecord name was already read
    pub fn add(&mut self, reader: &mut EsmReader) -> Result<(), EsmError> {
        let spell_id = reader.read_ref_id()?;
        self.list.push(spell_id);
        Ok(())
    }
    /// Just checks if the refid is in the list
    pub fn exists(&self, spell: &RefId) -> bool {
        self.list.contains(spell)
    }
}
