use crate::{RefId, StringRefId};

pub struct Attribute {
    // who knows what goes here
}
impl Attribute {
    pub const STRENGTH: RefId = RefId::SizedString(StringRefId("Strength"));
    pub const INTELLIGENCE: RefId = RefId::SizedString(StringRefId("Intelligence"));
    pub const WILLPOWER: RefId = RefId::SizedString(StringRefId("Willpower"));
    pub const AGILITY: RefId = RefId::SizedString(StringRefId("Agility"));
    pub const SPEED: RefId = RefId::SizedString(StringRefId("Speed"));
    pub const ENDURANCE: RefId = RefId::SizedString(StringRefId("Endurance"));
    pub const PERSONALITY: RefId = RefId::SizedString(StringRefId("Personality"));
    pub const LUCK: RefId = RefId::SizedString(StringRefId("Luck"));
    pub const NUM_ATTRIBUTES: usize = 8;

    // Array for indexing
    const ATTRIBUTES: [RefId; 8] = [
        Self::STRENGTH,
        Self::INTELLIGENCE,
        Self::WILLPOWER,
        Self::AGILITY,
        Self::SPEED,
        Self::ENDURANCE,
        Self::PERSONALITY,
        Self::LUCK,
    ];

    pub fn index_to_refid(index: i32) -> RefId {
        if index < 0 || index >= Self::NUM_ATTRIBUTES as i32 {
            return RefId::default();
        }
        Self::ATTRIBUTES.get(index as usize).unwrap().clone()
    }

    pub fn ref_id_to_index(id: &RefId) -> Option<usize> {
        Self::ATTRIBUTES.iter().position(|x| x == id)
    }
}
