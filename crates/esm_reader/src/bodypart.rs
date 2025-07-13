// src/bodypart.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct BodyPartData {
    pub part_sub_type: u8, // E.g., for hair: 0=long, 1=short, 2=bald; for face: 0-8, etc.
    pub is_vampire: u8,    // 0 = Not vampire, 1 = Vampire
    pub flags: u8,         // Bitflags: 0x1 = Female, 0x2 = Playable
    pub mesh_type: u8,     // Creature type: 0=Human, 1=Beast (Khajiit/Argonian), 2=Goblin, etc.
} // Total: 1+1+1+1 = 4 bytes

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BodyPart {
    pub id: RefId,             // From NAME subrecord (e.g., "b_n_dark elf_f_head_01")
    pub model: Option<String>, // From MODL subrecord (e.g., "b\\B_N_Dark Elf_F_Head_01.nif")
    pub body_part_type_code: Option<u8>, // From FNAM subrecord (main body part type, e.g., 0=Head, 1=Hair)
    pub data: Option<BodyPartData>,      // From BYDT subrecord
}

impl BodyPart {
    pub fn new() -> Self {
        Default::default()
    }

    /// Loads a BODY record's data from the EsmReader.
    /// Assumes the EsmReader is positioned after the "BODY" record name
    /// and its initial record header (size, flags) have been read.
    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut part = BodyPart::new();
        let mut id_found = false;
        let mut fnam_found = false; // Main part type
        let mut bydt_found = false; // Part characteristics
        let mut modl_found = false; // Model is essential

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    part.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    part.model = Some(reader.get_h_string()?);
                    modl_found = true;
                }
                b"FNAM" => {
                    // Body part type code
                    reader.get_sub_header()?; // Reads FNAM's data size into m_ctx.left_sub
                    let fnam_data_size = reader.get_sub_size();
                    if fnam_data_size >= 1 {
                        // Must have at least 1 byte for the type code
                        part.body_part_type_code = Some(reader.get_t::<u8>()?); // Read the first byte
                        fnam_found = true;
                        if fnam_data_size > 1 {
                            // If the file reports more data for FNAM than the expected 1 byte,
                            // skip the rest of this subrecord's declared data.
                            // An eprintln! warning could be added here for debugging.
                            reader.skip(fnam_data_size - 1)?;
                        }
                    } else {
                        // FNAM data size is 0, which is an error if FNAM is expected.
                        return Err(reader.report_sub_size_mismatch(1, fnam_data_size));
                    }
                }
                b"BYDT" => {
                    // Body part data
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<BodyPartData>() as u32; // 4 bytes
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    part.data = Some(reader.get_t::<BodyPartData>()?);
                    bydt_found = true;
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || part.id.empty() {
            return Err(reader.fail("BODY record missing mandatory NAME subrecord for its ID"));
        }
        if !modl_found {
            return Err(reader.fail(&format!(
                "BODY record '{}' missing MODL subrecord",
                part.id.to_debug_string()
            )));
        }
        if !fnam_found {
            return Err(reader.fail(&format!(
                "BODY record '{}' missing FNAM (part type) subrecord",
                part.id.to_debug_string()
            )));
        }
        if !bydt_found {
            return Err(reader.fail(&format!(
                "BODY record '{}' missing BYDT (part data) subrecord",
                part.id.to_debug_string()
            )));
        }

        Ok(part)
    }
}

// Enum for BodyPartType (FNAM values) - based on UESP for Morrowind
#[allow(dead_code)] // May not be used directly in parsing struct but useful for interpretation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyPartType {
    Head = 0,
    Hair = 1,
    Neck = 2,
    Cuirass = 3, // Chest in OpenMW source for ESM::PartReference, matches some contexts
    Groin = 4,
    LeftHand = 5, // Not LHand, distinct from ARMO/CLOT INDX values often
    RightHand = 6,
    LeftWrist = 7,
    RightWrist = 8,
    LeftForearm = 9,
    RightForearm = 10,
    LeftUpperArm = 11,
    RightUpperArm = 12,
    LeftFoot = 13,
    RightFoot = 14,
    LeftAnkle = 15,
    RightAnkle = 16,
    LeftKnee = 17,
    RightKnee = 18,
    LeftUpperLeg = 19,
    RightUpperLeg = 20,
    LeftPauldron = 21, // Clavicle in some older notes, Pauldron more common
    RightPauldron = 22,
    Weapon = 23, // Shield in some contexts for NPCs/Creatures, Weapon for player
    Tail = 24,
    Unknown = 255, // Default or error
}

impl From<u8> for BodyPartType {
    fn from(val: u8) -> Self {
        match val {
            0 => BodyPartType::Head,
            1 => BodyPartType::Hair,
            2 => BodyPartType::Neck,
            3 => BodyPartType::Cuirass,
            4 => BodyPartType::Groin,
            5 => BodyPartType::LeftHand,
            6 => BodyPartType::RightHand,
            7 => BodyPartType::LeftWrist,
            8 => BodyPartType::RightWrist,
            9 => BodyPartType::LeftForearm,
            10 => BodyPartType::RightForearm,
            11 => BodyPartType::LeftUpperArm,
            12 => BodyPartType::RightUpperArm,
            13 => BodyPartType::LeftFoot,
            14 => BodyPartType::RightFoot,
            15 => BodyPartType::LeftAnkle,
            16 => BodyPartType::RightAnkle,
            17 => BodyPartType::LeftKnee,
            18 => BodyPartType::RightKnee,
            19 => BodyPartType::LeftUpperLeg,
            20 => BodyPartType::RightUpperLeg,
            21 => BodyPartType::LeftPauldron,
            22 => BodyPartType::RightPauldron,
            23 => BodyPartType::Weapon,
            24 => BodyPartType::Tail,
            _ => BodyPartType::Unknown,
        }
    }
}

// Enum for MeshType (BYDT mType) - based on OpenMW ESM::BodyPart::MeshType
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyPartMeshType {
    Human = 0, // Includes Mer and human races
    Beast = 1, // Khajiit and Argonian
    // OpenMW also lists Goblin, Bear, Wolf, Dog, Horse, Dragon, SiltStrider, Deer, Hircine.
    // For simplicity, we'll stick to the ones most relevant for player/NPC part variation initially.
    // The byte in the file can hold other values.
    Goblin = 2,
    Bear = 3,
    Wolf = 4,
    Unknown = 255,
}

impl From<u8> for BodyPartMeshType {
    fn from(val: u8) -> Self {
        match val {
            0 => BodyPartMeshType::Human,
            1 => BodyPartMeshType::Beast,
            2 => BodyPartMeshType::Goblin,
            3 => BodyPartMeshType::Bear,
            4 => BodyPartMeshType::Wolf,
            _ => BodyPartMeshType::Unknown,
        }
    }
}
