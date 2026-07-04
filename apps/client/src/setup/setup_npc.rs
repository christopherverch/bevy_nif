use esm_parser_lib::{BodypartId, BodypartType, RefId, types::bodypart::BodypartFlags};
use esm_store::EsmStore;
use strum::EnumCount;
use strum_macros::EnumCount as EnumCountMacro;
/*
pub fn resolve_body_parts(
    race: &RefId,
    female: bool,
    first_person: bool,
    werewolf: bool,
    store: &EsmStore,
) -> Vec<Option<RefId>> {
    let mut parts = vec![None; PartReference::COUNT];

    // Filter valid parts
    let candidates = store.bodyparts.iter_shared().filter(|bp| {
        !bp.data.flags.contains(BodypartFlags::NOT_PLAYABLE)
            && bp.data.bodypart_type == BodypartType::Skin
            && bp.race == *race
    });

    for bodypart in candidates {
        let is_hand = bodypart.data.part == BodypartId::Hand;
        let is_same_gender = bodypart.data.flags.contains(BodypartFlags::FEMALE) == female;
        let part_is_fp = bodypart.is_first_person_bodypart();

        // 1. Handle First-Person Hand Fallback logic
        if first_person && is_hand && !part_is_fp {
            // Use functional approach to fill empty slots
            for target in get_part_references(bodypart.data.part) {
                if parts[target as usize].is_none() && is_same_gender {
                    parts[target as usize] = Some(bodypart.id.clone());
                }
            }
            continue;
        }

        // 2. View Mode enforcement
        if part_is_fp != first_person {
            continue;
        }

        // 3. Gender Fallback
        if female && !is_female(bodypart) {
            // Logic for filling missing slots with male parts
            // ...
            continue;
        }

        // 4. Exact match
        if female == is_female(bodypart) {
            for target in get_part_references(bodypart.part) {
                parts[target as usize] = Some(bodypart.id.clone());
            }
        }
    }
    dbg!(&parts);
    parts
}
*/
#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumCountMacro)]
pub enum PartReference {
    Head,
    Hair,
    Neck,
    Cuirass,
    Groin,
    RHand,
    LHand,
    RWrist,
    LWrist,
    RForearm,
    LForearm,
    RUpperarm,
    LUpperarm,
    RFoot,
    LFoot,
    RAnkle,
    LAnkle,
    RKnee,
    LKnee,
    RLeg,
    LLeg,
    Tail,
}
pub fn get_part_references(mesh_part: BodypartId) -> &'static [PartReference] {
    match mesh_part {
        BodypartId::Neck => &[PartReference::Neck],
        BodypartId::Chest => &[PartReference::Cuirass],
        BodypartId::Groin => &[PartReference::Groin],
        BodypartId::Hand => &[PartReference::RHand, PartReference::LHand],
        BodypartId::Wrist => &[PartReference::RWrist, PartReference::LWrist],
        BodypartId::Forearm => &[PartReference::RForearm, PartReference::LForearm],
        BodypartId::Upperarm => &[PartReference::RUpperarm, PartReference::LUpperarm],
        BodypartId::Foot => &[PartReference::RFoot, PartReference::LFoot],
        BodypartId::Ankle => &[PartReference::RAnkle, PartReference::LAnkle],
        BodypartId::Knee => &[PartReference::RKnee, PartReference::LKnee],
        BodypartId::Upperleg => &[PartReference::RLeg, PartReference::LLeg],
        BodypartId::Tail => &[PartReference::Tail],
        // Handle Head, Hair, and Clavicle as needed for your specific logic:
        BodypartId::Head | BodypartId::Hair | BodypartId::Clavicle => &[],
    }
}
