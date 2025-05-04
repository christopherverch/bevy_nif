use crate::nif::error::{ParseError, Result};
use crate::nif::parser::animation::read_text_key;
use crate::nif::parser::helpers::*;
use crate::nif::types::*;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

use crate::nif::types::NiTextKeyExtraData;

pub fn parse_nitextkeyextradata_fields(
    cursor: &mut Cursor<&[u8]>,
    block_index: u32,
) -> Result<NiTextKeyExtraData> {
    println!(
        "   Parsing NiTextKeyExtraData fields for block {}...",
        block_index
    );
    println!("cursor: {:x}", cursor.position());
    let base_data = parse_niextradata_fields(cursor)?;
    println!("cursor: {:x}", cursor.position());
    let num_keys = cursor.read_u32::<LittleEndian>()?;
    println!("     -> Num Text Keys: {}", num_keys);
    let mut keys_vec = Vec::with_capacity(num_keys as usize);
    if num_keys > 1000 {
        return Err(ParseError::InvalidData("Too many text keys".to_string()));
    }
    for _ in 0..num_keys {
        keys_vec.push(read_text_key(cursor)?);
    }
    println!("       -> Read {} text keys.", keys_vec.len());
    Ok(NiTextKeyExtraData {
        base: base_data,
        num_keys,
        keys: keys_vec,
    })
}
pub fn parse_niextradata_fields(cursor: &mut Cursor<&[u8]>) -> Result<NiExtraData> {
    println!("     Parsing NiExtraData fields...");
    let mut name_len = cursor.read_u32::<LittleEndian>()?;
    if name_len == 0xFFFFFFFF {
        name_len = 0;
    }
    let name = read_nif_string(cursor, name_len)?;
    println!("       -> Name: '{}'", name);
    let mut next_extra_data = None;
    if name_len > 0 {
        next_extra_data = read_link(cursor)?;
    }
    println!("       -> Next Extra Data: {:?}", next_extra_data);

    let _unknown_int_1 = cursor.read_u32::<LittleEndian>()?;
    Ok(NiExtraData {
        name,
        next_extra_data,
    })
}
