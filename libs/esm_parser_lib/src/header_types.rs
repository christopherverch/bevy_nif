// header_type.rs

use crate::Load;
use crate::common_types::{EsmVersion, FormatVersion, Name32, Name64};
use crate::esm_reader::{ConsumptionType, EsmError, EsmReader}; // Assuming esm_reader.rs is in the same crate/module

// --- Structs from loadtes3.hpp ---

#[derive(Debug, Clone, Default)]
pub struct Data {
    pub version: EsmVersion,
    pub file_type: i32, // 0=esp, 1=esm (Morrowind.esm should have 1)
    pub author: String,
    pub description: String,
    pub num_records: i32,
}

#[derive(Debug, Clone, Default)]
pub struct Gmdt {
    pub current_health: f32,
    pub maximum_health: f32,
    pub hour: f32,
    pub unknown1: [u8; 12],
    pub current_cell_name: Name64,
    pub unknown2: [u8; 4],
    pub player_name: Name32,
}

#[derive(Debug, Clone, Default)]
pub struct MasterData {
    pub name: String,
    pub size: u64,
}

/// \brief File header record (TES3)
#[derive(Debug, Clone, Default)]
pub struct Header {
    pub game_data: Gmdt,
    pub scrd: Vec<u8>,
    pub scrs: Vec<u8>,
    pub data: Data,
    pub format_version: FormatVersion,
    pub master_data: Vec<MasterData>,
}

impl Load for Header {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut header = Header::default();

        if reader.is_next_sub(Name32::new("HEDR"))? {
            reader.get_sub_header()?;
            header.data.version.ui = reader.read_as::<u32>(ConsumptionType::SubrecordPayload)?;
            header.data.file_type = reader.read_as::<i32>(ConsumptionType::SubrecordPayload)?;
            header.data.author = reader.get_string_view(32)?;
            header.data.description = reader.get_string_view(256)?;
            header.data.num_records = reader.read_as::<i32>(ConsumptionType::SubrecordPayload)?;
        }

        while reader.is_next_sub(Name32::new("MAST"))? {
            let mut master_data = MasterData::default();
            reader.get_sub_header()?;
            master_data.name =
                reader.get_string_view(reader.context.remaining_subrec_data as usize)?;

            if reader.is_next_sub(Name32::new("DATA"))? {
                reader.get_sub_header()?;
                master_data.size = reader.read_as::<u64>(ConsumptionType::SubrecordPayload)?;
            } else {
                return Err(reader.fail("Expected DATA subrecord after MAST"));
            }
            header.master_data.push(master_data);
        }

        if reader.is_next_sub(Name32::new("GMDT"))? {
            reader.get_sub_header()?;
            header.game_data.current_health =
                reader.read_as::<f32>(ConsumptionType::SubrecordPayload)?;
            header.game_data.maximum_health =
                reader.read_as::<f32>(ConsumptionType::SubrecordPayload)?;
            header.game_data.hour = reader.read_as::<f32>(ConsumptionType::SubrecordPayload)?;
            reader.read_exact(
                &mut header.game_data.unknown1,
                ConsumptionType::SubrecordPayload,
            )?;
            header.game_data.current_cell_name =
                reader.read_as::<Name64>(ConsumptionType::SubrecordPayload)?;
            reader.read_exact(
                &mut header.game_data.unknown2,
                ConsumptionType::SubrecordPayload,
            )?;
            header.game_data.player_name =
                reader.read_as::<Name32>(ConsumptionType::SubrecordPayload)?;
        }

        if reader.is_next_sub(Name32::new("SCRD"))? {
            reader.get_sub_header()?;
            let size = reader.context.remaining_subrec_data as usize;
            header.scrd.resize(size, 0);
            reader.read_exact(&mut header.scrd, ConsumptionType::SubrecordPayload)?;
        }

        if reader.is_next_sub(Name32::new("SCRS"))? {
            reader.get_sub_header()?;
            let size = reader.context.remaining_subrec_data as usize;
            header.scrs.resize(size, 0);
            reader.read_exact(&mut header.scrs, ConsumptionType::SubrecordPayload)?;
        }

        Ok(header)
    }
}

impl Header {
    #[allow(dead_code)]
    pub fn get_ver(&self) -> u32 {
        self.data.version.ui
    }
    #[allow(dead_code)]
    pub fn get_record_count(&self) -> i32 {
        self.data.num_records
    }
    #[allow(dead_code)]
    pub fn esm_version_f(&self) -> f32 {
        self.data.version.f
    }
    #[allow(dead_code)]
    pub fn get_author(&self) -> &str {
        &self.data.author
    }
    #[allow(dead_code)]
    pub fn get_description(&self) -> &str {
        &self.data.description
    }
}
