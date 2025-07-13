// header_type.rs

use crate::common_types::{EsmVersion, FormatVersion, Name, Name32, Name64};
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

impl Header {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn blank(&mut self) {
        self.game_data = Gmdt::default();
        self.scrd.clear();
        self.scrs.clear();
        self.data = Data::default();
        self.format_version = FormatVersion::default();
        self.master_data.clear();
    }

    pub fn from_reader(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut header = Self::new();

        reader.get_sub_name_is(Name::new("HEDR"))?;
        reader.get_sub_header()?; // This sets reader.m_ctx.left_sub with HEDR data size
        let hedr_size = reader.get_sub_size();

        // Uncomment for debugging HEDR parsing:
        // println!("[DEBUG Header::from_reader] Effective HEDR data size to parse: {}", hedr_size);
        // if reader.is_open() {
        //     if let Ok(pos) = reader.get_file_offset() {
        //          println!("[DEBUG Header::from_reader] Current file offset before HEDR data read: 0x{:X}", pos);
        //     }
        // }

        if hedr_size < 4 {
            // Minimum for version float
            return Err(reader.fail("HEDR subrecord too small for version float"));
        }
        let file_version_float: f32 = reader.get_f32_le(ConsumptionType::SubrecordPayload)?;
        // Uncomment for debugging:
        // println!("[DEBUG Header::from_reader] Version float: {}", file_version_float);
        header.data.version.f = file_version_float;

        if file_version_float == 1.0 || file_version_float == 1.2 || file_version_float == 1.3 {
            header.data.version.ui = 0x3F800000; // Common representation for 1.0 as u32
            header.format_version = FormatVersion(1); // Simplified
        } else {
            // This is a simplification. Real version mapping would be more complex.
            header.data.version.ui = file_version_float.to_bits();
            header.format_version = FormatVersion(file_version_float as u32); // Example
        }

        let mut bytes_read_in_hedr = 4u32; // Start with the size of the float we just read

        // **FIX: Read the file_type (int32_t) field**
        if hedr_size >= bytes_read_in_hedr + 4 {
            header.data.file_type = reader.get_i32_le(ConsumptionType::SubrecordPayload)?; // Reads an i32
            bytes_read_in_hedr += 4;
            // Uncomment for debugging:
            // println!("[DEBUG Header::from_reader] File Type: {}", header.data.file_type);
        } else {
            // Not enough data for file_type, this is an error if HEDR size implies it should be there
            // For robustness, we might allow it if hedr_size is very small (e.g. only version)
            // but for a standard Morrowind.esm HEDR (size 300), this would be an issue.
            if hedr_size >= 8 {
                // If size implies type should be there
                return Err(reader.fail("HEDR subrecord too small for file_type"));
            }
        }

        const AUTHOR_SIZE: usize = 32;
        if hedr_size >= bytes_read_in_hedr + AUTHOR_SIZE as u32 {
            let mut author_bytes = [0u8; AUTHOR_SIZE];
            reader.get_exact(&mut author_bytes, ConsumptionType::SubrecordPayload)?;
            // Uncomment for debugging:
            // println!("[DEBUG Header::from_reader] Author raw bytes (first 5): {:?}", &author_bytes[..std::cmp::min(5, AUTHOR_SIZE)]);
            header.data.author = String::from_utf8_lossy(&author_bytes)
                .trim_matches('\0')
                .to_string();
            bytes_read_in_hedr += AUTHOR_SIZE as u32;
        } else if hedr_size > bytes_read_in_hedr {
            let remaining = hedr_size - bytes_read_in_hedr;
            let mut temp_buf = vec![0u8; remaining as usize];
            reader.get_exact(&mut temp_buf, ConsumptionType::SubrecordPayload)?;
            header.data.author = String::from_utf8_lossy(&temp_buf)
                .trim_matches('\0')
                .to_string();
            bytes_read_in_hedr += remaining;
        }
        // Uncomment for debugging:
        // println!("[DEBUG Header::from_reader] Parsed Author: '{}'", header.data.author);

        const DESC_SIZE: usize = 256;
        if hedr_size >= bytes_read_in_hedr + DESC_SIZE as u32 {
            let mut desc_bytes = [0u8; DESC_SIZE];
            reader.get_exact(&mut desc_bytes, ConsumptionType::SubrecordPayload)?;
            // Uncomment for debugging:
            // println!("[DEBUG Header::from_reader] Desc raw bytes (first 5): {:?}", &desc_bytes[..std::cmp::min(5, DESC_SIZE)]);
            header.data.description = String::from_utf8_lossy(&desc_bytes)
                .trim_matches('\0')
                .to_string();
            bytes_read_in_hedr += DESC_SIZE as u32;
        } else if hedr_size > bytes_read_in_hedr {
            let remaining = hedr_size - bytes_read_in_hedr;
            let mut temp_buf = vec![0u8; remaining as usize];
            reader.get_exact(&mut temp_buf, ConsumptionType::SubrecordPayload)?;
            header.data.description = String::from_utf8_lossy(&temp_buf)
                .trim_matches('\0')
                .to_string();
            bytes_read_in_hedr += remaining;
        }
        // Uncomment for debugging:
        // let desc_preview = header.data.description.chars().take(20).collect::<String>();
        // println!("[DEBUG Header::from_reader] Parsed Description (first 20): '{}'", desc_preview);

        // Uncomment for debugging:
        // println!("[DEBUG Header::from_reader] Bytes read before num_records: {}, HEDR data size: {}", bytes_read_in_hedr, hedr_size);
        if hedr_size >= bytes_read_in_hedr + 4 {
            header.data.num_records = reader.get_i32_le(ConsumptionType::SubrecordPayload)?;
            bytes_read_in_hedr += 4;
        } else {
            // Uncomment for debugging:
            // println!("[DEBUG Header::from_reader] Condition for reading num_records was false or not enough bytes left.");
        }
        // Uncomment for debugging:
        // println!("[DEBUG Header::from_reader] Parsed Num Records: {}", header.data.num_records);

        if hedr_size > bytes_read_in_hedr {
            // Skip any remaining unexpected bytes in HEDR
            // Uncomment for debugging:
            // println!("[DEBUG Header::from_reader] Skipping {} remaining bytes in HEDR.", hedr_size - bytes_read_in_hedr);
            reader.skip_raw_bytes(
                (hedr_size - bytes_read_in_hedr) as i64,
                ConsumptionType::SubrecordPayload,
            )?;
        }

        while reader.has_more_subs() && reader.peek_next_sub(Name::new("MAST"))? {
            reader.get_sub_name_is(Name::new("MAST"))?;
            let master_name = reader.get_h_string()?;

            reader.get_sub_name_is(Name::new("DATA"))?;
            reader.get_sub_header()?;
            if reader.get_sub_size() != 8 {
                return Err(reader.fail("Expected DATA subrecord size of 8 for master file size"));
            }
            let master_size: u64 = reader.get_u64_le(ConsumptionType::SubrecordPayload)?;

            header.master_data.push(MasterData {
                name: master_name,
                size: master_size,
            });
        }

        if reader.has_more_subs() && reader.peek_next_sub(Name::new("GMDT"))? {
            reader.get_sub_name_is(Name::new("GMDT"))?;
            reader.get_sub_header()?;
            if reader.get_sub_size() as usize != std::mem::size_of::<Gmdt>() {
                return Err(reader.fail(&format!(
                    "GMDT size mismatch, expected {}, got {}",
                    std::mem::size_of::<Gmdt>(),
                    reader.get_sub_size()
                )));
            }
            header.game_data.current_health =
                reader.get_f32_le(ConsumptionType::SubrecordPayload)?;
            header.game_data.maximum_health =
                reader.get_f32_le(ConsumptionType::SubrecordPayload)?;
            header.game_data.hour = reader.get_f32_le(ConsumptionType::SubrecordPayload)?;
            reader.get_exact(
                &mut header.game_data.unknown1,
                ConsumptionType::SubrecordPayload,
            )?;
            header.game_data.current_cell_name =
                reader.get_name64_le(ConsumptionType::SubrecordPayload)?;
            reader.get_exact(
                &mut header.game_data.unknown2,
                ConsumptionType::SubrecordPayload,
            )?;
            header.game_data.player_name =
                reader.get_name32_le(ConsumptionType::SubrecordPayload)?;
        }

        if reader.has_more_subs() && reader.peek_next_sub(Name::new("SCRD"))? {
            reader.get_sub_name_is(Name::new("SCRD"))?;
            reader.get_sub_header()?;
            header.scrd.resize(reader.get_sub_size() as usize, 0);
            reader.get_exact(&mut header.scrd, ConsumptionType::SubrecordPayload)?;
        }

        if reader.has_more_subs() && reader.peek_next_sub(Name::new("SCRS"))? {
            reader.get_sub_name_is(Name::new("SCRS"))?;
            reader.get_sub_header()?;
            header.scrs.resize(reader.get_sub_size() as usize, 0);
            reader.get_exact(&mut header.scrs, ConsumptionType::SubrecordPayload)?;
        }
        Ok(header)
    }

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
    pub fn get_desc(&self) -> &str {
        &self.data.description
    }
    #[allow(dead_code)]
    pub fn get_game_files(&self) -> &Vec<MasterData> {
        &self.master_data
    }
    #[allow(dead_code)]
    pub fn get_format_version(&self) -> FormatVersion {
        self.format_version
    }
}
