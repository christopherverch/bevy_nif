use crate::{RefId, StringRefId};

pub struct Attribute {
    // who knows what goes her
}
impl Attribute {
    pub const NUM_ATTRIBUTES: usize = 8;
    pub fn index_to_refid(index: i32) -> RefId {
        if index < 0 || index >= Self::NUM_ATTRIBUTES as i32 {
            return RefId::default();
        }
        Self::ATTRIBUTE_IDS.get(index as usize).unwrap().clone()
    }

    const ATTRIBUTE_IDS: [RefId; Self::NUM_ATTRIBUTES] = [
        RefId::SizedString(StringRefId("Strength")),
        RefId::SizedString(StringRefId("Intelligence")),
        RefId::SizedString(StringRefId("Willpower")),
        RefId::SizedString(StringRefId("Agility")),
        RefId::SizedString(StringRefId("Speed")),
        RefId::SizedString(StringRefId("Endurance")),
        RefId::SizedString(StringRefId("Personality")),
        RefId::SizedString(StringRefId("Luck")),
    ];
}
