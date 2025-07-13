use crate::extra_data::{ExtraFields, NiStringExtraData};
use crate::nif::error::{ParseError, Result};
use crate::nif::parser::helpers::*;
use crate::nif::types::*;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

use crate::nif::types::NiTextKeyExtraData;

pub fn parse_nitextkeyextradata_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiTextKeyExtraData> {
    // 1. Parse the common base fields FIRST
    let extra_base = parse_extra_fields(cursor)?;

    // 2. Read fields specific to NiTextKeyExtraData
    let num_text_keys = cursor.read_u32::<LittleEndian>()?;

    // 3. Read Text Keys
    let mut text_keys = Vec::with_capacity(num_text_keys as usize);
    for i in 0..num_text_keys {
        let time = cursor.read_f32::<LittleEndian>()?;
        let text_len = cursor.read_u32::<LittleEndian>()?;
        let text = read_nif_string(cursor, text_len)?;

        text_keys.push(TextKey { time, value: text });

        // Optional logging/safety checks
        // ... (logging/safety checks as before) ...
        if (i + 1) % 100 == 0 || i == num_text_keys - 1 {}
        if text_len > 1024 * 1024 {
            return Err(ParseError::InvalidData(format!(
                "Key {} string length ({}) seems excessively large at offset 0x{:X}",
                i,
                text_len,
                cursor.position() - (text_len - 4) as u64
            )));
        }
    }

    let data = NiTextKeyExtraData {
        extra_base, // Assign the parsed base fields
        num_text_keys,
        text_keys,
    };

    Ok(data)
}

pub fn parse_extra_fields(cursor: &mut Cursor<&[u8]>) -> Result<ExtraFields> {
    let next_extra_data_link = read_link(cursor)?;
    let bytes_remaining_or_record_size = cursor.read_u32::<LittleEndian>()?;
    Ok(ExtraFields {
        next_extra_data_link,
        bytes_remaining_or_record_size,
    })
}
pub fn parse_nistringextradata_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiStringExtraData> {
    // 1. Parse the common base fields FIRST
    let extra_base = parse_extra_fields(cursor)?;

    // 2. Read the length-prefixed string data specific to this type
    let string_length = cursor.read_u32::<LittleEndian>()?;
    let string_data = read_nif_string(cursor, string_length)?;

    let data = NiStringExtraData {
        extra_base, // Assign the parsed base fields
        string_data,
    };

    Ok(data)
}
