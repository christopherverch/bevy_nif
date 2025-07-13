// ai_wander_data.rs (or could be part of a larger model/components.rs)

use crate::esm_reader::{ConsumptionType, EsmError, EsmReadable, EsmReader}; // Assuming esreader.rs // Assuming common_types.rs

// This trait will be implemented by structs that can be read as a whole
// from an ESM subrecord using the "decompose" pattern.

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AiWanderData {
    pub distance: i16,
    pub duration: i16,
    pub time_of_day: u8,
    pub idle: [u8; 8],
    pub should_repeat: u8, // bool might be more idiomatic if it's 0 or 1
}

impl EsmReadable for AiWanderData {
    fn read_members(reader: &mut EsmReader, expected_size: u32) -> Result<Self, EsmError> {
        let mut data = AiWanderData::default();

        // Calculate the actual size of the struct based on its members
        let actual_struct_size = (std::mem::size_of::<i16>() * 2)
            + (std::mem::size_of::<u8>() * 1)
            + (std::mem::size_of::<u8>() * 8)
            + (std::mem::size_of::<u8>() * 1);

        if expected_size as usize != actual_struct_size {
            return Err(reader.report_sub_size_mismatch_custom(
                expected_size as usize,
                actual_struct_size,
                "AiWanderData",
            ));
        }

        // Read members in the order they appear in the C++ struct / file format
        data.distance = reader.get_i16_le(ConsumptionType::SubrecordPayload)?;
        data.duration = reader.get_i16_le(ConsumptionType::SubrecordPayload)?;
        data.time_of_day = reader.get_u8_le(ConsumptionType::SubrecordPayload)?;

        // For fixed-size arrays like idle[8]
        for i in 0..8 {
            data.idle[i] = reader.get_u8_le(ConsumptionType::SubrecordPayload)?;
        }
        // Or, if get_t can handle arrays directly (which my current get_t cannot without changes):
        // data.idle = reader.get_t::<[u8; 8]>()?;
        // For now, manual loop is safer with the current get_t.

        data.should_repeat = reader.get_u8_le(ConsumptionType::SubrecordPayload)?;

        Ok(data)
    }
}

// Example of another struct that might use it, if you provide its C++ definition
// struct DurationData { ... }
// impl EsmReadable for DurationData { ... }
