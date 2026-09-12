use crate::{RefId, StringRefId};
/// Just so we can impl attribute so we access attributes like Attribute::Speed
pub struct Attribute;
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
        Attribute::STRENGTH,
        Attribute::INTELLIGENCE,
        Attribute::WILLPOWER,
        Attribute::AGILITY,
        Attribute::SPEED,
        Attribute::ENDURANCE,
        Attribute::PERSONALITY,
        Attribute::LUCK,
    ];

    pub fn attr_index_to_refid(index: i32) -> RefId {
        if index < 0 || index >= Attribute::NUM_ATTRIBUTES as i32 {
            return RefId::default();
        }
        Attribute::ATTRIBUTES.get(index as usize).unwrap().clone()
    }

    pub fn attr_ref_id_to_index(id: &RefId) -> Option<usize> {
        Attribute::ATTRIBUTES.iter().position(|x| x == id)
    }
}
