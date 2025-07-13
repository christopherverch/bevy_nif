// src/book.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct BookData {
    pub weight: f32,
    pub value: i32,
    pub scroll: i32,      // 0 for Book, 1 for Scroll
    pub skill_id: i32,    // Skill ID it teaches (-1 for none)
    pub enchant_pts: i32, // Enchantment points (for scrolls usually)
} // Total 20 bytes, but UESP says 24. OpenMW uses 20. Let's assume 20 based on fields.
// If it's 24, there are 4 padding bytes. The C++ OpenMW code reads these 5 fields sequentially.
// Let's check OpenMW's `ESM::Book::mData` size - it's these 5 fields.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Book {
    pub id: RefId,                  // From NAME subrecord
    pub model: Option<String>,      // From MODL subrecord
    pub full_name: Option<String>,  // From FNAM subrecord
    pub data: Option<BookData>,     // From BKDT subrecord
    pub icon: Option<String>,       // From ITEX subrecord
    pub text: Option<String>,       // From TEXT subrecord
    pub script: Option<RefId>,      // From SCRI subrecord
    pub enchantment: Option<RefId>, // From ENAM subrecord
}

impl Book {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut book = Book::new();
        let mut id_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;
            // println!("  [BOOK DEBUG] Subrecord: {}", sub_name.to_string_lossy());

            match &sub_name.m_data {
                b"NAME" => {
                    book.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    book.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    book.full_name = Some(reader.get_h_string()?);
                }
                b"BKDT" => {
                    reader.get_sub_header()?;
                    // Size of BookData: 5 fields * 4 bytes/field = 20 bytes
                    let expected_bkdt_size = std::mem::size_of::<BookData>() as u32;
                    if reader.get_sub_size() != expected_bkdt_size {
                        return Err(reader
                            .report_sub_size_mismatch(expected_bkdt_size, reader.get_sub_size()));
                    }
                    let mut bk_data = BookData::default();
                    bk_data.weight = reader.get_t::<f32>()?;
                    bk_data.value = reader.get_t::<i32>()?;
                    bk_data.scroll = reader.get_t::<i32>()?;
                    bk_data.skill_id = reader.get_t::<i32>()?;
                    bk_data.enchant_pts = reader.get_t::<i32>()?;
                    book.data = Some(bk_data);
                }
                b"ITEX" => {
                    book.icon = Some(reader.get_h_string()?);
                }
                b"TEXT" => {
                    book.text = Some(reader.get_h_string()?);
                }
                b"SCRI" => {
                    book.script = Some(reader.get_ref_id()?);
                }
                b"ENAM" => {
                    book.enchantment = Some(reader.get_ref_id()?);
                }
                _ => {
                    // println!("    [BOOK DEBUG] Skipping unknown subrecord: {}", sub_name.to_string_lossy());
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || book.id.empty() {
            return Err(reader.fail("BOOK record missing mandatory NAME subrecord for its ID"));
        }
        // Other fields like FNAM, MODL, BKDT are generally expected for a valid book.
        if book.full_name.is_none() {
            return Err(reader.fail(&format!(
                "BOOK record '{}' missing FNAM subrecord",
                book.id.to_debug_string()
            )));
        }
        if book.data.is_none() {
            return Err(reader.fail(&format!(
                "BOOK record '{}' missing BKDT subrecord",
                book.id.to_debug_string()
            )));
        }

        Ok(book)
    }
}
