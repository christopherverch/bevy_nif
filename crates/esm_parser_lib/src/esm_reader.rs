use bytemuck::Pod;
use num_traits::FromPrimitive;
use to_utf8::Utf8Encoder;

use crate::common_types::{EsmContext, StringSizeType};
use crate::format_version_defs::{
    MAX_LIMITED_SIZE_STRINGS_FORMAT_VERSION, MAX_STRING_REF_ID_FORMAT_VERSION,
    MAX_USE_ESM_CELL_ID_FORMAT_VERSION,
};
use crate::header_types::Header;
use crate::name_defs::RecNameBytes;
use crate::readers_cache::ReadersCache;
use crate::refid_types::{FormId, RefId, RefIdType};
use crate::types::flags::ObjectFlags;
use crate::{
    CellId, ESM3ExteriorCellRefId, EmptyRefId, GeneratedRefId, IndexRefId, Load, Name4, StringRefId,
};

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str;
static EMPTY_STRING: &str = "";

// All consumption types also consume the types above them
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumptionType {
    /// Reading data that's only counted against the total file size
    FileOnly,
    /// Reading data that is part of a record's content
    RecordContent,
    /// Reading data that is part of a subrecord's payload.
    SubrecordPayload,
}

pub trait ReadSeek: Read + Seek + Send + Sync {}
impl<T: Read + Seek + Send + Sync> ReadSeek for T {}

pub struct EsmReader {
    pub(crate) file: BufReader<Box<dyn ReadSeek>>,
    pub(crate) file_size: u64,
    pub(crate) file_pos: u64,
    pub(crate) context: EsmContext,
    pub(crate) record_flags: ObjectFlags,
    pub(crate) text_decoder: Utf8Encoder,
    pub(crate) header: Header,
    pub(crate) buffer: Vec<u8>,
    /// Set when game is actually loaded, per-file mapping based on the existing file index state
    /// ex: mod1.esm is loaded, says mod0 should be at index 0. mod2.esm loads, says mod0 should be
    /// at index 1, we check the existing state and see mod0 is at index 0, so we map 1->0
    /// Note none of this happens in the reader, but rather in a main game state loading.
    pub(crate) content_file_mapping: Option<HashMap<i16, i16>>,
}

impl EsmReader {
    pub fn new(text_decoder: Utf8Encoder) -> Self {
        let reader = EsmReader {
            file: BufReader::new(Box::new(io::empty())),
            file_size: 0,
            file_pos: 0,
            context: EsmContext::default(),
            record_flags: ObjectFlags::empty(),
            header: Header::default(),
            text_decoder,
            buffer: vec![0; 50 * 1024],
            content_file_mapping: None,
        };
        reader
    }
    pub fn get_header(&self) -> &Header {
        &self.header
    }
    fn get_remaining_file_data(&self) -> Result<u64, EsmError> {
        match self.file_size.checked_sub(self.file_pos) {
            Some(remaining) => Ok(remaining),
            None => Err(self.fail("EsmReader file_pos went past end of file!")),
        }
    }

    fn open_raw(&mut self, path: &Path) -> Result<(), EsmError> {
        self.close();
        let mut file = Box::new(File::open(path)?) as Box<dyn ReadSeek>;
        self.file_size = file.seek(SeekFrom::End(0))?;
        file.seek(SeekFrom::Start(0))?;

        self.file = BufReader::new(file);
        self.context.filename = path.to_owned();

        Ok(())
    }
    /// Opens the esm file and reads the rec header and the header
    pub fn open(&mut self, path: &Path) -> Result<(), EsmError> {
        self.open_raw(path)?;
        if self.read_rec_name()? != Name4::new("TES3") {
            return Err(self.fail("Not a valid Morrowind file"));
        }
        self.read_rec_header()?;
        self.header = self.load()?;
        Ok(())
    }

    pub fn set_encoder(&mut self, encoder: Utf8Encoder) {
        self.text_decoder = encoder;
    }

    pub fn get_cloned_context(&mut self) -> Result<EsmContext, EsmError> {
        self.file_pos = self.file.stream_position()?;
        self.context.file_pos = self.file_pos;
        Ok(self.context.clone())
    }
    pub fn restore_context(&mut self, saved_context: &EsmContext) -> Result<(), EsmError> {
        if self.context.filename != saved_context.filename {
            self.open_raw(&saved_context.filename)?;
        }
        // Clone the context and seek to the context's file position
        self.context = saved_context.clone();
        self.file.seek(SeekFrom::Start(self.context.file_pos))?;
        self.file_pos = saved_context.file_pos;
        Ok(())
    }

    /// Resets the esm reader to an empty state
    pub fn close(&mut self) {
        self.file = BufReader::new(Box::new(io::empty()));
        self.context.clear();
        self.file_size = 0;
        self.record_flags = ObjectFlags::empty();
        self.file_pos = 0;
        self.header = Header::default();
    }

    /// Resolves master file names to file indices in the readers cache.
    ///
    /// Matches each master from the header against earlier entries in the cache,
    /// storing the matched indices in `parent_file_indices`.
    pub fn resolve_parent_file_indices(
        &mut self,
        readers_cache: Rc<RefCell<ReadersCache>>,
    ) -> Result<(), EsmError> {
        // Clear any existing resolved indices
        self.context.parent_file_indices.clear();
        let index = self.get_index() as usize;
        let readers_cache_borrowed = readers_cache.borrow();
        for master in &self.header.master_data {
            let fname = &master.name;
            // Default to current file index
            let mut matched_index = index;
            // Search earlier files in the load order for a matching filename
            for i in 0..index {
                if readers_cache_borrowed.get_file_size(i)? == 0 {
                    continue; // Skip invalid or empty files
                }
                // Get the filename of the candidate file from the cache.
                let candidate_path = readers_cache_borrowed.get_name(i)?;
                if let Some(candidate_fname_os) = candidate_path.file_name() {
                    if let Some(candidate_fname) = candidate_fname_os.to_str() {
                        // Perform case-insensitive comparison.
                        if fname.eq_ignore_ascii_case(candidate_fname) {
                            matched_index = i;
                            break;
                        }
                    }
                }
            }
            // Store the resolved index (or self.get_index() if not found)
            self.context.parent_file_indices.push(matched_index);
        }

        Ok(())
    }
    pub fn get_parent_file_indices(&self) -> &[usize] {
        &self.context.parent_file_indices
    }
    /// Remaps an index to what the mapping says the actual index is
    /// if mod0 was saved with mod1 at index 1, but mod1 was loaded at index 0,
    /// the mapping should map 1->0
    pub fn apply_content_file_mapping(&mut self, form_id: &mut FormId) -> bool {
        if let Some(mapping) = &self.content_file_mapping {
            if let Some(content_file) = form_id.content_file {
                if let Some(&actual_content_file) = mapping.get(&content_file) {
                    form_id.content_file = Some(actual_content_file);
                    return true;
                } else {
                    return false;
                }
            }
        }
        true
    }

    pub fn read_cell_id(&mut self) -> Result<RefId, EsmError> {
        if self.header.format_version <= MAX_USE_ESM_CELL_ID_FORMAT_VERSION {
            let cell_id: CellId = self.load()?;
            if cell_id.paged {
                Ok(RefId::ESM3ExteriorCell(ESM3ExteriorCellRefId::new(
                    cell_id.index.x,
                    cell_id.index.y,
                )))
            } else {
                Ok(RefId::UnsizedString(StringRefId::new(&cell_id.world_space)))
            }
        } else {
            self.read_header_refid_check_name(Name4::new("NAME"))
        }
    }

    pub fn read_header_refid_check_name(
        &mut self,
        expected_name: Name4,
    ) -> Result<RefId, EsmError> {
        self.get_or_read_validated_subname(expected_name)?;
        return self.read_refid();
    }
    /// If we are above MAX_LIMITED_SIZE_STRINGS_FORMAT_VERSION then read the size from the file,
    /// otherwise use the size function parameter
    pub fn read_maybe_fixed_string_size(&mut self, size: u32) -> Result<&str, EsmError> {
        let mut final_size = size;

        // Check the file format version to see if the string size is explicitly stored.
        if self.header.format_version > MAX_LIMITED_SIZE_STRINGS_FORMAT_VERSION {
            // Read the stored size from the file.
            let stored_size: u32 = self.read_typed(ConsumptionType::FileOnly)?;

            final_size = stored_size;
        }
        // Validate that the stored size fits within the remaining subrecord data.
        if final_size > self.context.remaining_subrec_data {
            return Err(EsmError::StringSizeMismatch {
                stored: final_size,
                remaining: self.context.remaining_subrec_data,
            });
        }
        // Read the string using the determined size.
        let string_view = self.read_decode_str(final_size as usize)?;

        Ok(string_view)
    }

    pub fn get_maybe_fixed_refid_sized(&mut self, size: u32) -> Result<RefId, EsmError> {
        if self.header.format_version <= MAX_STRING_REF_ID_FORMAT_VERSION {
            // For older formats, RefIds are simple strings of a fixed size.
            let string_val = self.read_maybe_fixed_string_size(size)?;
            Ok(RefId::from_str(string_val))
        } else {
            // For newer formats, the RefId can be a variant type determined by size.
            self.read_refid_impl(self.context.remaining_subrec_data as usize)
        }
    }
    pub fn get_file_size(&self) -> usize {
        self.file_size as usize
    }
    pub fn read_typed<T: Pod>(&mut self, consumption_type: ConsumptionType) -> Result<T, EsmError> {
        let mut data: T = T::zeroed();
        // Get a mutable slice of the bytes of T.
        let slice = bytemuck::bytes_of_mut(&mut data);
        self.read_exact(slice, consumption_type)?;

        Ok(data)
    }
    pub fn read_exact(
        &mut self,
        buffer: &mut [u8],
        consumption_type: ConsumptionType,
    ) -> Result<(), EsmError> {
        EsmReader::_read_exact_impl(
            &mut self.file,
            &mut self.context,
            buffer,
            consumption_type,
            &mut self.file_pos,
        )
    }
    fn _read_exact_impl(
        file: &mut BufReader<Box<dyn ReadSeek>>,
        context: &mut EsmContext,
        buffer: &mut [u8],
        consumption_type: ConsumptionType,
        file_pos: &mut u64,
    ) -> Result<(), EsmError> {
        let len = buffer.len();
        match consumption_type {
            // File pos is incremented regardless, so we don't have to do anything here
            ConsumptionType::FileOnly => {}
            ConsumptionType::RecordContent => {
                if context.remaining_rec_data < len as u32 {
                    return Err(EsmReader::report_sub_size_mismatch(
                        context.remaining_rec_data,
                        len as u32,
                        context,
                        *file_pos,
                    ));
                }
                context.remaining_rec_data -= len as u32;
            }
            ConsumptionType::SubrecordPayload => {
                if context.remaining_subrec_data < len as u32 {
                    return Err(EsmReader::report_sub_size_mismatch(
                        context.remaining_subrec_data,
                        len as u32,
                        context,
                        *file_pos,
                    ));
                }
                context.remaining_rec_data -= len as u32;
                context.remaining_subrec_data -= len as u32;
            }
        }
        file.read_exact(buffer)?;
        *file_pos += len as u64;
        Ok(())
    }
    pub fn skip(&mut self, bytes: usize) -> Result<(), EsmError> {
        self.file.seek_relative(bytes as i64)?;
        self.file_pos += bytes as u64;
        Ok(())
    }

    pub(crate) fn report_sub_size_mismatch(
        expected: u32,
        actual: u32,
        context: &EsmContext,
        file_pos: u64,
    ) -> EsmError {
        EsmError::SubSizeMismatch {
            actual,
            expected,
            sub_name: context.sub_name,
            rec_name: context.rec_name,
            file: context.filename.clone(),
            offset: file_pos,
        }
    }

    pub fn get_sub_name(&self) -> Name4 {
        self.context.sub_name
    }
    pub fn get_sub_size(&self) -> u32 {
        self.context.remaining_subrec_data
    }
    pub fn compare_sub_size_to_expected(&self, expected_size: u32) -> Result<(), EsmError> {
        let actual_size = self.get_sub_size();
        if actual_size != expected_size {
            return Err(EsmReader::report_sub_size_mismatch(
                expected_size,
                actual_size,
                &self.context,
                self.file_pos,
            ));
        }

        Ok(())
    }
    pub fn get_record_flags(&self) -> ObjectFlags {
        self.record_flags
    }
    pub fn get_index(&self) -> i16 {
        self.context.index
    }
    pub fn set_index(&mut self, index: i16) {
        self.context.index = index;
    }

    pub fn get_file_offset(&mut self) -> Result<u64, EsmError> {
        self.file.stream_position().map_err(EsmError::from)
    }

    /// Skips `count` bytes in the underlying stream and updates remaining_file_data.
    pub(crate) fn skip_raw_bytes(
        &mut self,
        offset: i64,
        consumption_type: ConsumptionType,
    ) -> Result<(), EsmError> {
        if offset == 0 {
            return Ok(());
        }

        let abs_offset_as_u64 = offset.abs() as u64;

        // --- 1. Pre-checks based on direction and consumption_type ---
        if offset > 0 {
            // Forward skip
            // Check against remaining_file_data
            if self.get_remaining_file_data()? < abs_offset_as_u64 {
                return Err(self.fail(&format!(
                            "EOF (file budget): Cannot skip forward {} bytes (have {}). Context: {:?}, File pos: 0x{:X}",
                            offset, self.get_remaining_file_data()?, consumption_type, self.file_pos
                )));
            }
            // Check if we have remaining data for whatever consumptiontype skip we have
            match consumption_type {
                ConsumptionType::RecordContent => {
                    if self.context.remaining_rec_data < abs_offset_as_u64 as u32 {
                        return Err(self.fail(&format!(
                                    "EOF (record budget): Cannot skip forward {} bytes (have {}). Context: {:?}, File pos: 0x{:X}",
                                    offset, self.context.remaining_rec_data, consumption_type, self.file_pos
                        )));
                    }
                }
                ConsumptionType::SubrecordPayload => {
                    if self.context.remaining_rec_data < abs_offset_as_u64 as u32 {
                        return Err(self.fail(&format!("EOF (record for subrec): Cannot skip forward {} bytes (have {}). Context: {:?}, File pos: 0x{:X}",
                                    offset,
                                    self.context.remaining_rec_data,
                                    consumption_type,
                                    self.file_pos)));
                    }
                    if self.context.remaining_subrec_data < abs_offset_as_u64 as u32 {
                        return Err(self.fail(&format!("EOF (subrecord budget): Cannot skip forward {} bytes (have {}). Context: {:?}, File pos: 0x{:X}",
                                    offset,
                                    self.context.remaining_subrec_data,
                                    consumption_type,
                                    self.file_pos)));
                    }
                }
                ConsumptionType::FileOnly => {} // No additional checks
            }
        } else {
            // Backward skip (offset < 0)
            if self.file_pos < abs_offset_as_u64 {
                return Err(self.fail(&format!(
                            "Seek error: Cannot skip backward {} bytes from 0x{:X} (would go before start of file). Context: {:?}.",
                            offset, self.file_pos, consumption_type
                )));
            }
        }

        // --- 2. Seek ---
        let original_tracked_pos = self.file_pos;
        let new_stream_pos = self.file.seek(SeekFrom::Current(offset)).map_err(|e| {
            self.fail(&format!(
                "IO error during seek by {} (consumption_type {:?} at 0x{:X}): {}",
                offset, consumption_type, original_tracked_pos, e
            ))
        })?;

        // --- 3. Validate Seek (ensure it moved as expected) ---
        let expected_new_pos = if offset > 0 {
            original_tracked_pos.checked_add(abs_offset_as_u64)
        } else {
            // offset < 0
            original_tracked_pos.checked_sub(abs_offset_as_u64)
        };

        match expected_new_pos {
            Some(expected_pos) if new_stream_pos == expected_pos => {
                // Seek was successful and had no issues
            }
            _ => {
                // Seek didn't land where expected (clamped at EOF/BOF, or arithmetic issue)
                return Err(self.fail(&format!(
                            "Skip error: Seek by {} did not result in expected position. Original pos: 0x{:X}, Stream reported new pos: 0x{:X}, Expected new pos: 0x{:X?}. Context: {:?}.",
                            offset, original_tracked_pos, new_stream_pos, expected_new_pos, consumption_type
                )));
            }
        }
        // --- 4. Update All Counters ---
        self.file_pos = new_stream_pos;

        if offset > 0 {
            match consumption_type {
                ConsumptionType::RecordContent => {
                    self.context.remaining_rec_data -= abs_offset_as_u64 as u32;
                }
                ConsumptionType::SubrecordPayload => {
                    self.context.remaining_rec_data -= abs_offset_as_u64 as u32;
                    self.context.remaining_subrec_data -= abs_offset_as_u64 as u32;
                }
                ConsumptionType::FileOnly => {}
            }
        } else {
            // offset < 0 (Backward skip)
            match consumption_type {
                ConsumptionType::RecordContent => {
                    self.context.remaining_rec_data += abs_offset_as_u64 as u32;
                    // Add back
                }
                ConsumptionType::SubrecordPayload => {
                    self.context.remaining_rec_data += abs_offset_as_u64 as u32; // Add back
                    self.context.remaining_subrec_data += abs_offset_as_u64 as u32;
                    // Add back
                }
                // Already updated with self.file_pos = new_stream_pos
                ConsumptionType::FileOnly => {}
            }
        }

        Ok(())
    }
    // --- Record Navigation ---
    pub fn has_more_recs(&self) -> Result<bool, EsmError> {
        const MINIMAL_RECORD_HEADER_SIZE: u64 = 16; // For TES3: Name(4) + Size(4) + Ignored(4) + Flags(4)
        Ok(self.get_remaining_file_data()? >= MINIMAL_RECORD_HEADER_SIZE)
    }

    pub fn read_rec_name(&mut self) -> Result<Name4, EsmError> {
        if !self.has_more_recs()? {
            return Err(self.fail("No more records"));
        }
        if self.has_more_subs() {
            return Err(self.fail("Previous record contains unread bytes"));
        }
        if self.context.remaining_rec_data != 0 {
            // Maybe should throw an error?
            self.skip_raw_bytes(
                self.context.remaining_rec_data.into(),
                ConsumptionType::RecordContent,
            )?
        }

        let mut name_bytes = [0u8; Name4::S_CAPACITY];
        self.read_exact(&mut name_bytes, ConsumptionType::FileOnly)?;
        self.context.rec_name = Name4 { data: name_bytes };
        self.context.sub_cached = false;
        Ok(self.context.rec_name)
    }

    pub fn read_rec_header(&mut self) -> Result<u32, EsmError> {
        if self.get_remaining_file_data()? < (3 * size_of::<u32>()) as u64 {
            return Err(self.fail("EOF while reading record header"));
        }
        let data_size: u32 = self.read_typed(ConsumptionType::FileOnly)?;
        let _unused_header_data: u32 = self.read_typed(ConsumptionType::FileOnly)?;
        let flags: u32 = self.read_typed(ConsumptionType::FileOnly)?;

        self.context.remaining_rec_data = data_size; // This is the size of the data block following this header

        // Check if the declared data size exceeds what's left in the file after accounting for this header
        if self.context.remaining_rec_data as u64 > self.get_remaining_file_data()? {
            return Err(self.fail(&format!("Record data size ({}) is invalid or exceeds remaining file data ({}) for record '{}'",
                        self.context.remaining_rec_data, self.get_remaining_file_data()?, self.context.rec_name.to_string_lossy())));
        }

        self.record_flags = ObjectFlags::from_bits_truncate(flags);
        Ok(flags)
    }

    pub fn skip_record(&mut self) -> Result<(), EsmError> {
        let remaining_rec_data = &self.context.remaining_rec_data;
        self.skip_raw_bytes(*remaining_rec_data as i64, ConsumptionType::RecordContent)?;
        self.context.sub_cached = false;
        Ok(())
    }

    pub fn has_more_subs(&self) -> bool {
        self.context.remaining_rec_data > 0 || self.context.sub_cached
    }

    pub fn get_or_read_sub_name(&mut self) -> Result<Name4, EsmError> {
        if self.context.sub_cached {
            self.context.sub_cached = false;
            return Ok(self.context.sub_name);
        }
        if self.context.remaining_rec_data < Name4::S_CAPACITY as u32 {
            return Err(self.fail(&format!(
                        "End of record (remaining_rec_data: {}) before reading sub-record name for rec '{}'",
                        self.context.remaining_rec_data,
                        self.context.rec_name.to_string_lossy()
            )));
        }
        let mut name_bytes = [0u8; Name4::S_CAPACITY];
        self.read_exact(&mut name_bytes, ConsumptionType::RecordContent)?;
        self.context.sub_name = Name4 { data: name_bytes };
        Ok(self.context.sub_name)
    }

    pub fn read_sub_header(&mut self) -> Result<(), EsmError> {
        if self.context.remaining_rec_data < 4 {
            return Err(self.fail(&format!(
                    "End of record (remaining_rec_data: {}) before full sub-record header (need {} bytes) for rec '{}'. File pos: 0x{:X}",
                        self.context.remaining_rec_data,
                        4,
                        self.context.rec_name.to_string_lossy(),
                        self.file_pos
            )));
        }
        let sub_data_payload_size: u32 = self.read_typed(ConsumptionType::RecordContent)?;
        self.context.remaining_subrec_data = sub_data_payload_size;

        // check the declared subrecord data payload size is within what's left in the whole record
        if self.context.remaining_subrec_data > self.context.remaining_rec_data {
            return Err(self.fail(&format!(
                        "Subrecord data payload size ({}) for sub '{}' in rec '{}' exceeds remaining record data ({}). Subrecord header ends at file_pos approx 0x{:X}.",
                        self.context.remaining_subrec_data,
                        self.context.sub_name.to_string_lossy(),
                        self.context.rec_name.to_string_lossy(),
                        self.context.remaining_rec_data,
                        self.file_pos
            )));
        }

        Ok(())
    }
    pub fn read_header_string_with_name(&mut self, expected_name: Name4) -> Result<&str, EsmError> {
        self.get_or_read_validated_subname(expected_name)?;
        self.read_header_decode_str()
    }
    pub fn get_or_read_validated_subname(&mut self, expected_name: Name4) -> Result<(), EsmError> {
        let actual_name = self.get_or_read_sub_name()?;
        if actual_name != expected_name {
            self.context.sub_cached = true;
            return Err(EsmError::SubRecordMismatch {
                expected: expected_name,
                found: actual_name,
            });
        }
        Ok(())
    }

    pub fn is_next_sub(&mut self, name_to_check: Name4) -> Result<bool, EsmError> {
        if !self.has_more_subs() {
            return Ok(false);
        }
        let actual_name = self.get_or_read_sub_name()?;
        if actual_name == name_to_check {
            // Matched. Name4 consumed from stream (if not cached), remaining_rec_data updated, sub_cached is false.
            Ok(true)
        } else {
            self.context.sub_cached = true; // Cache the name that was read but didn't match
            Ok(false)
        }
    }

    pub fn peek_next_sub(&mut self, name_to_check: Name4) -> Result<bool, EsmError> {
        if !self.has_more_subs() {
            return Ok(false);
        }
        // Reads the sub name (advances stream) or just gets it if it was cached
        let sub_name = self.get_or_read_sub_name()?;
        self.context.sub_cached = true; // Mark it as cached, since we're doing a peek
        Ok(sub_name == name_to_check)
    }

    pub fn cache_sub_name(&mut self) {
        self.context.sub_cached = true;
    }
    pub fn skip_sub_data(&mut self) -> Result<(), EsmError> {
        self.read_sub_header()?;
        self.skip_sub_data_payload()?;
        self.context.sub_cached = false;
        Ok(())
    }
    // --- Data Consumption from Subrecords ---
    /// Skips the data payload of the current subrecord.
    /// Assumes read_sub_header() was called and context.remaining_subrec_data is set.
    fn skip_sub_data_payload(&mut self) -> Result<(), EsmError> {
        let count = self.context.remaining_subrec_data;
        if count > 0 {
            self.skip_raw_bytes(count as i64, ConsumptionType::SubrecordPayload)?;
        }
        Ok(())
    }

    /// Reads exact buffer from subrecord payload. Consumes from remaining_subrec_data & remaining_rec_data.
    /// Assumes read_sub_header() was called and context.remaining_subrec_data is set.
    fn read_sub_data_exact(&mut self, buf: &mut [u8]) -> Result<(), EsmError> {
        let size = buf.len() as u32;
        if size > self.context.remaining_subrec_data {
            return Err(self.fail(&format!(
                "read_sub_data_exact: read {} from sub '{}' with {} left",
                size,
                self.context.sub_name.to_string_lossy(),
                self.context.remaining_subrec_data
            )));
        }
        self.read_exact(buf, ConsumptionType::SubrecordPayload)?;
        Ok(())
    }

    /// Reads string from subrecord's data payload. Consumes all of context.remaining_subrec_data.
    /// Assumes read_sub_header() was called and context.remaining_subrec_data is set.
    pub fn read_decode_str(&mut self, size_to_read: usize) -> Result<&str, EsmError> {
        // Resize buffer if needed
        if self.buffer.len() <= size_to_read {
            self.buffer.resize(3 * size_to_read, 0);
        }
        // Ensure zero termination
        self.buffer[size_to_read] = 0;

        // Read bytes into buffer
        let target_buffer_slice = &mut self.buffer[..size_to_read];
        if size_to_read > 0 {
            // use read_exact_impl so we don't double borrow self, since we already borrowed buffer
            EsmReader::_read_exact_impl(
                &mut self.file,
                &mut self.context,
                target_buffer_slice,
                ConsumptionType::SubrecordPayload,
                &mut self.file_pos,
            )?;
        }

        // Find first null byte to determine string length
        let actual_str_len = target_buffer_slice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(size_to_read);

        // Now get a slice of the buffer up to the null byte
        let raw_bytes_slice = &target_buffer_slice[..actual_str_len];

        // Decode to utf8

        Ok(self.text_decoder.get_utf8(raw_bytes_slice))
    }

    /// Reads a string subrecord that has a header.
    pub fn read_header_decode_str(&mut self) -> Result<&str, EsmError> {
        self.read_sub_header()?;

        // This is a special case check for zero-length strings in some older mods,
        // which still contain a single zero byte in the payload.
        if self.header.format_version <= MAX_STRING_REF_ID_FORMAT_VERSION
            && self.context.remaining_subrec_data == 0
            && self.has_more_subs()
            && self.peek_byte()? == Some(0)
        {
            // Skip the following zero byte
            self.skip_raw_bytes(1, ConsumptionType::RecordContent)?;
            Ok(EMPTY_STRING)
        } else {
            // Otherwise, read the string with the size from the subrecord header.
            self.read_decode_str(self.context.remaining_subrec_data as usize)
        }
    }
    pub fn read_header_decode_string_validate_subname(
        &mut self,
        name: Name4,
    ) -> Result<String, EsmError> {
        self.get_or_read_validated_subname(name)?;
        self.read_header_decode_string()
    }
    pub fn read_header_decode_string(&mut self) -> Result<String, EsmError> {
        Ok(self.read_header_decode_str()?.to_string())
    }
    pub fn get_optional_named_header_string(&mut self, name: Name4) -> Result<&str, EsmError> {
        if self.is_next_sub(name)? {
            self.read_header_decode_str()
        } else {
            Ok(EMPTY_STRING)
        }
    }
    pub fn read_refid_named_sub_or_default(&mut self, name: Name4) -> Result<RefId, EsmError> {
        if self.is_next_sub(name)? {
            self.read_refid()
        } else {
            Ok(RefId::default())
        }
    }
    pub fn read_typed_named_header_opt<T: Pod>(
        &mut self,
        name: Name4,
    ) -> Result<Option<T>, EsmError> {
        if self.is_next_sub(name)? {
            Ok(Some(self.read_typed_header()?))
        } else {
            Ok(None)
        }
    }
    pub fn read_typed_named_header<T>(&mut self, name: Name4) -> Result<T, EsmError>
    where
        T: Load,
    {
        self.get_or_read_validated_subname(name)?;
        self.read_sub_header()?;
        Ok(self.load()?)
    }
    /// For Pod types
    pub fn read_typed_header<T: Pod>(&mut self) -> Result<T, EsmError> {
        self.read_typed_header_common(|this| this.read_typed(ConsumptionType::SubrecordPayload))
    }

    /// for any type that has a load impl
    pub fn read_typed_header_load<T>(&mut self) -> Result<T, EsmError>
    where
        T: Load,
    {
        self.read_typed_header_common(|this| this.load())
    }
    fn read_typed_header_common<T, F>(&mut self, read_fn: F) -> Result<T, EsmError>
    where
        F: FnOnce(&mut Self) -> Result<T, EsmError>,
    {
        self.read_sub_header()?;
        if (self.context.remaining_subrec_data as usize) < size_of::<T>() {
            return Err(EsmReader::report_sub_size_mismatch(
                size_of::<T>() as u32,
                self.context.remaining_subrec_data,
                &self.context,
                self.file_pos,
            ));
        }
        read_fn(self)
    }

    pub fn read_sub_header_and_load<T: Load>(&mut self) -> Result<T, EsmError> {
        self.read_sub_header()?;
        self.load()
    }
    pub fn skip_named_header_refid(&mut self, name: Name4) -> Result<(), EsmError> {
        if self.is_next_sub(name)? {
            self.skip_header_refid()?;
        }
        Ok(())
    }
    fn skip_header_refid(&mut self) -> Result<(), EsmError> {
        Ok(self.skip_header_string()?)
    }
    pub fn skip_header_string(&mut self) -> Result<(), EsmError> {
        self.read_sub_header()?;

        // This is a special case check for zero-length strings in some older mods,
        // which still contain a single zero byte in the payload.
        if self.header.format_version <= MAX_STRING_REF_ID_FORMAT_VERSION
            && self.context.remaining_subrec_data == 0
            && self.has_more_subs()
            && self.peek_byte()? == Some(0)
        {
            // Skip the following zero byte
            self.skip_raw_bytes(1, ConsumptionType::RecordContent)?;
            return Ok(());
        }

        // Otherwise, skip the string with the size from the subrecord header.
        self.skip_raw_bytes(
            self.context.remaining_subrec_data as i64,
            ConsumptionType::SubrecordPayload,
        )?;

        Ok(())
    }
    pub fn skip_sub_header(&mut self) -> Result<(), EsmError> {
        self.read_sub_header()?;

        let sub_size = self.context.remaining_subrec_data;
        self.skip_raw_bytes(sub_size as i64, ConsumptionType::SubrecordPayload)?;
        Ok(())
    }
    pub fn skip_sub_header_sized(&mut self, expected_size: u32) -> Result<(), EsmError> {
        self.skip_sub_header()?;

        let sub_size = self.context.remaining_subrec_data;

        if sub_size as u32 != expected_size {
            return Err(Self::report_sub_size_mismatch(
                expected_size,
                sub_size,
                &self.context,
                self.file_pos,
            ));
        }
        Ok(())
    }
    pub fn skip_sub_header_until(&mut self, name: Name4) -> Result<(), EsmError> {
        while self.has_more_subs() && !self.is_next_sub(name)? {
            self.context.sub_cached = false;
            self.skip_sub_header()?;
        }
        if self.has_more_subs() {
            self.context.sub_cached = true;
        }
        Ok(())
    }

    /// Peeks at the next byte in the stream without advancing the file position.
    /// Returns None if at the end of the file.
    fn peek_byte(&mut self) -> Result<Option<u8>, EsmError> {
        let mut buf = [0u8; 1];
        let pos = self.file_pos;
        let read_count = self.file.read(&mut buf)?;
        self.file.seek(io::SeekFrom::Start(pos))?;
        self.file_pos = pos;

        if read_count == 1 {
            Ok(Some(buf[0]))
        } else {
            Ok(None)
        }
    }
    pub fn read_sub_header_validate_name<const EXPECTED_SIZE: u32>(
        &mut self,
        tag: Name4,
    ) -> Result<(), EsmError> {
        self.get_or_read_validated_subname(tag)?;
        self.read_sub_header()?;
        let actual_size = self.context.remaining_subrec_data;
        if actual_size != EXPECTED_SIZE {
            return Err(EsmError::SubSizeMismatch {
                sub_name: tag,
                file: self.context.filename.clone(),
                expected: EXPECTED_SIZE,
                actual: actual_size,
                rec_name: self.context.rec_name,
                offset: self.file.stream_position()?,
            });
        }
        Ok(())
    }

    /// This reads a 32-bit FormId from the stream. This is typically used for the main record header.
    pub fn read_form_id(&mut self, wide: bool, tag: Name4) -> Result<FormId, EsmError> {
        let mut result = FormId::default();

        if wide {
            const EXPECTED_SIZE: u32 = 8;
            self.read_sub_header_validate_name::<EXPECTED_SIZE>(tag)?;
            result.index = self.load()?;
            let content_file_index: i32 = self.load()?;
            // Sentinel handling and i32->i16 conversion
            // Assumed safe that we can put the i32 into an i16 because morrowind never has content
            // file index > 256
            result.content_file = if content_file_index == -1 {
                None
            } else {
                Some(content_file_index as i16)
            };
        } else {
            const EXPECTED_SIZE: u32 = 4;
            self.read_sub_header_validate_name::<EXPECTED_SIZE>(tag)?;
            result.index = self.load()?;
        }

        Ok(result)
    }
    pub fn read_refid_sized(&mut self, size: usize) -> Result<RefId, EsmError> {
        if self.header.format_version <= MAX_STRING_REF_ID_FORMAT_VERSION {
            // For older formats, the RefId is a simple string.
            let s = self.read_header_decode_str()?;
            Ok(RefId::from_str(s))
        } else {
            // For newer formats, the sub-header is read directly in this function,
            // and then read_refid_impl() handles the RefId variants.
            self.read_sub_header()?;
            self.read_refid_impl(size)
        }
    }
    pub fn read_refid(&mut self) -> Result<RefId, EsmError> {
        if self.header.format_version <= MAX_STRING_REF_ID_FORMAT_VERSION {
            return Ok(RefId::from_str(self.read_header_decode_str()?));
        }
        self.read_sub_header()?;
        self.read_refid_impl(self.context.remaining_subrec_data as usize)
    }

    pub(crate) fn read_refid_impl(&mut self, size: usize) -> Result<RefId, EsmError> {
        let refid_type_byte: u8 = self.read_typed(ConsumptionType::FileOnly)?;
        let refid_type = RefIdType::try_from(refid_type_byte)?;

        match refid_type {
            RefIdType::Empty => Ok(RefId::Empty(EmptyRefId)),
            RefIdType::UnsizedString => {
                if size < size_of::<RefIdType>() {
                    return Err(self.fail(&format!(
                        "Requested RefId record size is too small ({} < {})",
                        size,
                        size_of::<RefIdType>()
                    )));
                }
                let s =
                    self.read_decode_str((size - size_of::<RefIdType>()).try_into().unwrap())?;
                Ok(RefId::from_str(s))
            }
            RefIdType::SizedString => {
                let min_size = size_of::<RefIdType>() + size_of::<StringSizeType>();
                if size < min_size {
                    return Err(self.fail(&format!(
                        "Requested RefId record size is too small ({} < {})",
                        size, min_size
                    )));
                }
                let stored_size: StringSizeType =
                    self.read_typed(ConsumptionType::SubrecordPayload)?;
                let max_size = size - min_size;
                if stored_size as usize > max_size {
                    return Err(self.fail(&format!(
                        "RefId string does not fit subrecord size ({} > {})",
                        stored_size, max_size
                    )));
                }
                let s = self.read_decode_str(stored_size.try_into().unwrap())?;
                Ok(RefId::from_str(s))
            }
            RefIdType::FormId => {
                let mut form_id = self.load()?;
                if self.apply_content_file_mapping(&mut form_id) {
                    Ok(RefId::FormId(form_id))
                } else {
                    Ok(RefId::Empty(EmptyRefId))
                }
            }
            RefIdType::Generated => {
                let generated: u64 = self.read_typed(ConsumptionType::FileOnly)?;
                Ok(RefId::Generated(GeneratedRefId::new(generated)))
            }
            RefIdType::Index => {
                let record_type_u32: u32 = self.read_typed(ConsumptionType::FileOnly)?;
                let record_type = RecNameBytes::from_u32(record_type_u32)
                    .ok_or_else(|| EsmError::UnknownRecName(record_type_u32))?;
                let index: u32 = self.read_typed(ConsumptionType::FileOnly)?;
                Ok(RefId::Index(IndexRefId::new(record_type, index)))
            }
            RefIdType::ESM3ExteriorCell => {
                let x: i32 = self.read_typed(ConsumptionType::FileOnly)?;
                let y: i32 = self.read_typed(ConsumptionType::FileOnly)?;
                Ok(RefId::ESM3ExteriorCell(ESM3ExteriorCellRefId::new(x, y)))
            }
        }
    }

    pub(crate) fn fail(&self, msg: &str) -> EsmError {
        EsmError::Format(format!(
            "{}\n  File: {}\n  Record: {}\n  Subrecord: {}\n  Offset: 0x{:X}",
            msg,
            self.context.filename.display(),
            self.context.rec_name.to_string_lossy(),
            self.context.sub_name.to_string_lossy(),
            self.file_pos
        ))
    }

    pub fn report_sub_size_mismatch_custom(
        &self,
        got_from_file: usize,
        expected_by_struct: usize,
        struct_name: &str,
    ) -> EsmError {
        self.fail(&format!(
            "Subrecord size mismatch for {}: from file header {}, struct expects {}",
            struct_name, got_from_file, expected_by_struct
        ))
    }
    /// Reads a string subrecord and normalizes path separators to forward slashes.
    pub fn read_normalized_path_string(&mut self) -> Result<String, EsmError> {
        let path_str = self.read_header_decode_str()?;
        Ok(path_str.replace('\\', "/"))
    }
    pub fn load<L>(&mut self) -> Result<L, EsmError>
    where
        L: Load,
    {
        L::load(self)
    }
}

#[derive(Debug)]
pub enum EsmError {
    Io(io::Error),
    Format(String),
    SubRecordMismatch {
        expected: Name4,
        found: Name4,
    },
    StringSizeMismatch {
        stored: u32,
        remaining: u32,
    },
    SubSizeMismatch {
        actual: u32,
        expected: u32,
        sub_name: Name4,
        rec_name: Name4,
        file: PathBuf,
        offset: u64,
    },
    UnknownRecName(u32),
    InvalidRefIdType(u8),
    InvalidFormIdConversion(String),
    Custom(String),
}
impl From<io::Error> for EsmError {
    fn from(err: io::Error) -> EsmError {
        EsmError::Io(err)
    }
}
impl std::fmt::Display for EsmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EsmError::Io(e) => write!(f, "IO Error: {}", e),
            EsmError::Format(msg) => write!(f, "ESM Format Error: {}", msg),
            EsmError::SubRecordMismatch { expected, found } => write!(
                f,
                "Subrecord Mismatch: Expected '{}', found '{}'",
                expected.to_string_lossy(),
                found.to_string_lossy()
            ),
            EsmError::SubSizeMismatch {
                actual,
                expected,
                sub_name,
                rec_name,
                file,
                offset,
            } => write!(
                f,
                "Subrecord Size Mismatch in file '{}', record '{}', sub-record '{}' (offset 0x{:X}): Actual {}, Expected {}",
                file.display(),
                rec_name.to_string_lossy(),
                sub_name.to_string_lossy(),
                offset,
                actual,
                expected
            ),
            EsmError::StringSizeMismatch { stored, remaining } => write!(
                f,
                "String size mismatch: stored: {}, Remaining: {}",
                stored, remaining
            ),
            EsmError::UnknownRecName(rec_bytes) => {
                write!(f, "Unknown record name: {:?}", rec_bytes)
            }
            EsmError::InvalidRefIdType(refid_type) => {
                write!(f, "Invalid RefId Type: {}", refid_type)
            }
            EsmError::InvalidFormIdConversion(msg) => write!(f, "FormId Conversion Error: {}", msg),
            EsmError::Custom(msg) => write!(f, "ESM Error: {}", msg),
        }
    }
}
impl std::error::Error for EsmError {}
