//#![allow(dead_code)]
// esreader.rs (Rewritten with borrow checker fixes)

use bytemuck::Pod;

use crate::common_types::{
    EsmContext, FormatVersion, MAX_STRING_REF_ID_FORMAT_VERSION_CONST, Name, StringSizeType,
};
use crate::header_types::Header;
use crate::readers_cache::ReadersCache;
use crate::refid_types::{FormId, RefId, RefIdType};
use crate::types::flags::ObjectFlags;
use crate::{Load, Name32, Name64};

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::fs::File;
use std::io::{self, BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str; // For character encoding transformations

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumptionType {
    /// Reading data that's only accounted against the total file size
    FileOnly,
    /// Reading data that is part of a record's content
    RecordContent,
    /// Reading data that is part of a subrecord's payload.
    SubrecordPayload,
}

pub trait ReadSeek: Read + Seek + Send + Sync {}
impl<T: Read + Seek + Send + Sync> ReadSeek for  T {}

pub trait TextDecoder: Send + Sync + 'static {
    fn get_utf8(&self, input: &[u8]) -> Result<String, EsmError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Windows1252Decoder;
impl TextDecoder for Windows1252Decoder {
    fn get_utf8(&self, input: &[u8]) -> Result<String, EsmError> {
        let (decoded_cow, _encoding_used, _had_errors) = encoding_rs::WINDOWS_1252.decode(input);
        Ok(decoded_cow.into_owned())
    }
}


pub struct EsmReader {
    pub(crate) file: BufReader<Box<dyn ReadSeek>>,
    pub(crate) file_size: u64,
    pub(crate) file_pos: u64,
    pub(crate) context: EsmContext,
    pub(crate) record_flags: ObjectFlags,
    pub(crate) text_decoder: Box<dyn TextDecoder>,
    pub(crate) header: Header,
    pub(crate) buffer: Vec<u8>,
    pub(crate) content_file_mapping: Option<HashMap<u8, u8>>,
}


    pub trait EsmReadable: Sized + Default {
        /// Loads the fields of this struct from the reader.
        /// The reader is assumed to be positioned at the start of the subrecord's data,
        /// after its name and size have been read by `get_sub_header`.
        /// The `expected_size` is the size reported by the subrecord header.
        fn read_members(reader: &mut EsmReader, expected_size: u32) -> Result<Self, EsmError>;
    }
impl EsmReader{
    pub fn new() -> Result<Self, EsmError> {
        let reader = EsmReader {
            file: BufReader::new(Box::new(io::empty())),
            file_size: 0,
            file_pos: 0,
            context: EsmContext::default(),
            record_flags: ObjectFlags::empty(),
            header: Header::default(),
            text_decoder: Box::new(Windows1252Decoder),
            buffer: vec![0; 50 * 1024],
            content_file_mapping: None,

        };
        Ok(reader)
    }

    pub fn open_raw(&mut self, path: &Path) -> Result<(), EsmError> {
        self.close();
        let mut file = Box::new(File::open(path)?) as Box<dyn ReadSeek>;
        self.file_size = file.seek(SeekFrom::End(0))?;
        file.seek(SeekFrom::Start(0))?;

        self.file = BufReader::new(file);
        self.context.filename = path.to_owned();

        let rec_name = self.read_as::<Name32>(ConsumptionType::FileOnly)?;
        if rec_name != Name32::new("TES3") {
            return Err(self.fail("Expected a TES3 header record"));
        }
        let record_size = self.read_as::<u32>(ConsumptionType::FileOnly)? as usize;
        self.context.remaining_rec_data = record_size as u32;

        self.header = self.load()?;
        Ok(())
    } 
    pub fn load<L>(&mut self) -> Result<L, EsmError>
    where
        L: Load,
    {
        L::load(self)
    }

    pub fn get_context(&mut self) -> Result<EsmContext, EsmError> {
        self.file_pos =self.file.stream_position()?;
        Ok(self.context.clone())
    }
  pub fn restore_context(&mut self, rc: &EsmContext) -> Result<(), EsmError> {
        if self.context.filename != rc.filename {
            self.open_raw(&rc.filename)?;
        }
        // Copy the context and seek to the context's file position
        self.context = rc.clone();
        self.file.seek(SeekFrom::Start(self.context.file_pos))?;
        Ok(())
    }

    pub fn close(&mut self) {
        // Reset the file reader to an empty state
        self.file = BufReader::new(Box::new(io::empty()));
        self.context.clear();
        self.file_size = 0;
        self.record_flags = ObjectFlags::empty();
        self.file_pos = 0;
        self.header = Header::default();
    }
    pub fn resolve_parent_file_indices(&mut self, readers_cache: Rc<RefCell<ReadersCache>>) -> Result<(), EsmError> {
        self.context.parent_file_indices.clear();
        let index = self.get_index();
        let readers_cache_borrowed = readers_cache.borrow();
        for master in &self.header.master_data {
            let fname = &master.name;
            let mut matched_index = index;
            for i in 0..index {
                // Check if the file at this index is valid (non-zero size).
                if readers_cache_borrowed.get_file_size(i)? == 0 {
                    continue;
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
            self.context.parent_file_indices.push(matched_index);
        }

        Ok(())
    }

    pub fn get_file_size(&self) -> usize {
        self.file_size as usize
    }
    pub fn read_as<T: Pod>(&mut self, consumption_type: ConsumptionType) -> Result<T, EsmError> {
        let size = std::mem::size_of::<T>();
        let mut buf = vec![0u8; size];
        self.read_exact(buf.as_mut(), consumption_type)?;
        Ok(bytemuck::cast_slice(&buf)[0])
    } 
    pub fn read_exact(
        &mut self,
        buffer: &mut [u8],
        consumption_type: ConsumptionType,
    ) -> Result<(), EsmError> {
        EsmReader::_read_exact_impl(&mut self.file, &mut self.context, buffer, consumption_type,&mut self.file_pos)
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
                            context,
                            context.remaining_rec_data,
                            len as u32,
                    ));
                }
                context.remaining_rec_data -= len as u32;
            }
            ConsumptionType::SubrecordPayload => {
                if context.remaining_subrec_data < len as u32 {
                    return Err(EsmReader::report_sub_size_mismatch(
                            context,
                            context.remaining_subrec_data,
                            len as u32,
                    ));
                }
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

    pub fn apply_content_file_mapping(&mut self, form_id: &mut FormId) -> bool {
        if let Some(mapping) = &self.content_file_mapping {
            if let Some(content_file_index) = form_id.content_file_index{
                if let Some(&parent_index) = mapping.get(&content_file_index) {
                    form_id.content_file_index = Some(parent_index);
                    return true;
                }
                else {
                    return false;
                }

            }
        }
        true
    }
    pub(crate) fn report_sub_size_mismatch(context: &EsmContext, expected: u32, actual: u32) -> EsmError {
        EsmError::SubSizeMismatch {
            actual,
            expected,
            sub_name: context.sub_name,
            rec_name: context.rec_name,
            file: context.filename.clone(),
            offset: context.file_pos,
        }
    }




    pub fn ret_sub_name(&self) -> Name {
        self.context.sub_name
    }
    pub fn get_sub_size(&self) -> u32 {
        self.context.remaining_subrec_data
    }
    pub fn compare_sub_size_to_expected(&self, expected_size: u32) -> Result<(),EsmError>
    {
        let actual_size = self.get_sub_size();
        if actual_size != expected_size {
            return Err(EsmReader::report_sub_size_mismatch(&self.context,expected_size, actual_size));
        }

        Ok(())
    }
    pub fn get_record_flags(&self) -> ObjectFlags {
        self.record_flags
    }
    pub fn get_index(&self) -> usize {
        self.context.index
    }
    pub fn set_index(&mut self, index: usize) {
        self.context.index = index;
    }

    pub fn get_file_offset(&mut self) -> Result<u64, EsmError> {
        self.file
            .stream_position()
            .map_err(EsmError::from)
    }





    // Read a string with the given sub-record name
    pub fn get_header_name_string(&mut self, name: Name)
    {

    }
    pub fn read_subrecord_header(&mut self) -> Result<(Name, u32), EsmError> {
        if !self.has_more_subs() {
            return Err(self.fail("No more subrecords to read"));
        }
        let sub_name = self.get_sub_name()?;
        let sub_size = self.get_sub_size();
        Ok((sub_name, sub_size))
    }
    /// Skips `count` bytes in the underlying stream and updates `context.file_pos` and
    /// updates remaining_file_data.
    pub(crate) fn skip_raw_bytes(
        &mut self,
        offset: i64,
        consumption_type: ConsumptionType,
    ) -> Result<(), EsmError> {
        if offset == 0 {
            return Ok(());
        }

        let abs_offset_as_u64 = offset.abs() as u64;
        let abs_offset_for_rec_data = offset.abs() as u64; 
        let abs_offset_for_subrec_data = offset.abs() as u64; 

        // --- 1. Pre-checks based on direction and consumption_type ---
        if offset > 0 { // Forward skip
                        // Check against remaining_file_data (fundamental check)
            if self.context.remaining_file_data < abs_offset_as_u64 {
                return Err(self.fail(&format!(
                            "EOF (file budget): Cannot skip forward {} bytes (have {}). Context: {:?}, File pos: 0x{:X}",
                            offset, self.context.remaining_file_data, consumption_type, self.context.file_pos
                )));
            }
            // Context-specific budget checks for forward skip
            match consumption_type {
                ConsumptionType::RecordContent => {
                    if self.context.remaining_rec_data < abs_offset_for_rec_data as u32 {
                        return Err(self.fail(&format!(
                                    "EOF (record budget): Cannot skip forward {} bytes (have {}). Context: {:?}, File pos: 0x{:X}",
                                    offset, self.context.remaining_rec_data, consumption_type, self.context.file_pos
                        )));
                    }
                }
                ConsumptionType::SubrecordPayload => {
                    if self.context.remaining_rec_data < abs_offset_for_rec_data as u32{
                        return Err(self.fail(&format!("EOF (record for subrec): Cannot skip forward {} bytes (have {}). Context: {:?}, File pos: 0x{:X}", offset, self.context.remaining_rec_data, consumption_type, self.context.file_pos)));
                    }
                    if self.context.remaining_subrec_data < abs_offset_for_subrec_data as u32 {
                        return Err(self.fail(&format!("EOF (subrecord budget): Cannot skip forward {} bytes (have {}). Context: {:?}, File pos: 0x{:X}", offset, self.context.remaining_subrec_data, consumption_type, self.context.file_pos)));
                    }
                }
                ConsumptionType::FileOnly => {} // No additional checks
            }
        } else { // Backward skip (offset < 0)
            if self.context.file_pos < abs_offset_as_u64 {
                return Err(self.fail(&format!(
                            "Seek error: Cannot skip backward {} bytes from 0x{:X} (would go before start of file). Context: {:?}.",
                            offset, self.context.file_pos, consumption_type
                )));
            }
        }

        // --- 2. Perform Seek ---
        let original_tracked_pos = self.context.file_pos;
        let new_stream_pos = self.file.seek(SeekFrom::Current(offset)).map_err(|e| {
            self.fail(&format!("IO error during seek by {} (consumption_type {:?} at 0x{:X}): {}", offset, consumption_type, original_tracked_pos, e))
        })?;

        // --- 3. Validate Seek (ensure it moved precisely as expected) ---
        let expected_new_pos = if offset > 0 {
            original_tracked_pos.checked_add(abs_offset_as_u64)
        } else { // offset < 0
            original_tracked_pos.checked_sub(abs_offset_as_u64)
        };

        match expected_new_pos {
            Some(expected_pos) if new_stream_pos == expected_pos => {
                // Seek was successful and precise
            }
            _ => { // Seek didn't land where expected (clamped at EOF/BOF, or arithmetic issue)
                return Err(self.fail(&format!(
                            "Skip error: Seek by {} did not result in expected position. Original pos: 0x{:X}, Stream reported new pos: 0x{:X}, Expected new pos: 0x{:X?}. Context: {:?}.",
                            offset, original_tracked_pos, new_stream_pos, expected_new_pos, consumption_type
                )));
            }
        }
        // --- 4. Update All Counters ---
        self.context.file_pos = new_stream_pos; // Most reliable position

        if offset > 0 {
            self.context.remaining_file_data -= abs_offset_as_u64;
            match consumption_type {
                ConsumptionType::RecordContent => {
                    self.context.remaining_rec_data -= abs_offset_for_rec_data as u32;
                }
                ConsumptionType::SubrecordPayload => {
                    self.context.remaining_rec_data -= abs_offset_for_rec_data as u32;
                    self.context.remaining_subrec_data -= abs_offset_for_subrec_data as u32;
                }
                ConsumptionType::FileOnly => {}
            }
        } else { // offset < 0 (Backward skip)
            self.context.remaining_file_data += abs_offset_as_u64; // Add back to remaining file data
            match consumption_type {
                ConsumptionType::RecordContent => {
                    self.context.remaining_rec_data += abs_offset_for_rec_data as u32; // Add back
                }
                ConsumptionType::SubrecordPayload => {
                    self.context.remaining_rec_data += abs_offset_for_rec_data as u32; // Add back
                    self.context.remaining_subrec_data += abs_offset_for_subrec_data as u32; // Add back
                }
                ConsumptionType::FileOnly => {}
            }
        }

        Ok(())
    }
    // --- Record Navigation ---
    pub fn has_more_recs(&self) -> bool {
        const MINIMAL_RECORD_HEADER_SIZE: u64 = 16; // For TES3: Name(4) + Size(4) + Ignored(4) + Flags(4)
        self.context.remaining_file_data >= MINIMAL_RECORD_HEADER_SIZE
    }

    pub fn get_rec_name(&mut self) -> Result<Name, EsmError> {
        if !self.has_more_recs() {
            return Err(self.fail("No more records"));
        }
        if self.context.remaining_rec_data != 0 {
            self.skip_raw_bytes(self.context.remaining_rec_data.into(), ConsumptionType::RecordContent)?
        }

        let mut name_bytes = [0u8; Name::S_CAPACITY];
        self.read_exact(&mut name_bytes, ConsumptionType::FileOnly)?;
        self.context.rec_name = Name { data: name_bytes };
        self.context.sub_cached = false;
        Ok(self.context.rec_name)
    }

    fn get_rec_header_flags_internal_and_return(&mut self) -> Result<u32, EsmError> {
        if self.context.remaining_file_data < 12 {
            return Err(self.fail("EOF while reading record header"));
        }
        let data_size : u32 = self.read_as(ConsumptionType::FileOnly)?;
        let _unused_header : u32 = self.read_as(ConsumptionType::FileOnly)?;
        let flags_val : u32 = self.read_as(ConsumptionType::FileOnly)?;

        self.context.remaining_rec_data = data_size; // This is the size of the data block *following* this header

        // Check if the declared data size exceeds what's left in the file *after accounting for this header*
        if self.context.remaining_rec_data < 0
            || self.context.remaining_rec_data
                > self.context.remaining_file_data.try_into().unwrap()
        {
            return Err(self.fail(&format!("Record data size ({}) is invalid or exceeds remaining file data ({}) for record '{}'", 
                        self.context.remaining_rec_data, self.context.remaining_file_data, self.context.rec_name.to_string_lossy())));
        }
        // `remaining_file_data` will be further decremented by `remaining_rec_data` when skip_record or equivalent processing happens.
        Ok(flags_val)
    }

    pub fn get_rec_header(&mut self) -> Result<u32, EsmError> {
        let flags = self.get_rec_header_flags_internal_and_return()?;
        self.record_flags = ObjectFlags::from_bits_truncate(flags);
        Ok(flags)
    }

    pub fn skip_record(&mut self) -> Result<(), EsmError> {
        let bytes_to_skip_in_data_block = self.context.remaining_rec_data;
        self.skip_raw_bytes(bytes_to_skip_in_data_block as i64,ConsumptionType::RecordContent)?;
        // Now that the data block of the record is skipped/accounted for, adjust remaining_file_data.
        self.context.sub_cached = false;
        Ok(())
    }

    pub fn has_more_subs(&self) -> bool {
        self.context.remaining_rec_data > 0 || self.context.sub_cached
    }

    pub fn get_sub_name(&mut self) -> Result<Name, EsmError> {
        if self.context.sub_cached {
            self.context.sub_cached = false;
            return Ok(self.context.sub_name);
        }
        if self.context.remaining_rec_data < Name::S_CAPACITY as i64 {
            return Err(self.fail(&format!(
                        "End of record (remaining_rec_data: {}) before reading sub-record name for rec '{}'",
                        self.context.remaining_rec_data,
                        self.context.rec_name.to_string_lossy()
            )));
        }
        let mut name_bytes = [0u8; Name::S_CAPACITY];
        self.read_exact(&mut name_bytes,ConsumptionType::RecordContent)?;
        self.context.sub_name = Name { data: name_bytes };
        Ok(self.context.sub_name)
    }

    pub fn get_sub_header(&mut self) -> Result<(), EsmError> {
        if self.context.remaining_rec_data < 4 {
            return Err(self.fail(&format!(
                        "End of record (remaining_rec_data: {}) before full sub-record header (need {} bytes) for rec '{}'. File pos: 0x{:X}",
                        self.context.remaining_rec_data,
                        4,
                        self.context.rec_name.to_string_lossy(),
                        self.context.file_pos
            )));
        }
        let sub_data_payload_size : u32 = self.read_as(ConsumptionType::RecordContent)?;
        self.context.remaining_subrec_data = sub_data_payload_size;


        // Validate the declared subrecord data payload size against what's *truly* left
        if self.context.remaining_subrec_data > self.context.remaining_rec_data {
            return Err(self.fail(&format!(
                        "Subrecord data payload size ({}) for sub '{}' in rec '{}' exceeds remaining record data ({}). Subrecord header ends at file_pos approx 0x{:X}.",
                        self.context.remaining_subrec_data,
                        self.context.sub_name.to_string_lossy(),
                        self.context.rec_name.to_string_lossy(),
                        self.context.remaining_rec_data,
                        self.context.file_pos 
            )));
        }


        Ok(())
    }
    pub fn get_sub_name_is(&mut self, expected_name: Name) -> Result<(), EsmError> {
        let actual_name = self.get_sub_name()?;
        if actual_name == expected_name {
            Ok(())
        } else {
            self.context.sub_cached = true;
            Err(EsmError::SubRecordMismatch {
                expected: expected_name,
                found: actual_name,
            })
        }
    }

    pub fn is_next_sub(&mut self, name_to_check: Name) -> Result<bool, EsmError> {
        if !self.has_more_subs() {
            return Ok(false);
        }

        let actual_name = self.get_sub_name()?;
        if actual_name == name_to_check {
            // Matched. Name consumed from stream (if not cached), remaining_rec_data updated, sub_cached is false.
            Ok(true)
        } else {
            self.context.sub_cached = true; // Cache the name that was read but didn't match
            Ok(false)
        }
    }

    pub fn peek_next_sub(&mut self, name_to_check: Name) -> Result<bool, EsmError> {
        if !self.has_more_subs() {
            return Ok(false);
        }
        let _ = self.get_sub_name()?; // Ensure context.sub_name is populated (advances stream if not cached)
        self.context.sub_cached = true; // Crucially, mark it as cached for the *next* operation.
        Ok(self.context.sub_name == name_to_check)
    }

    pub fn cache_sub_name(&mut self) {
        self.context.sub_cached = true;
    }
    pub fn skip_sub_data(&mut self) -> Result<(), EsmError> {
        self.get_sub_header()?;
        self.skip_sub_data_payload()?;
        self.context.sub_cached = false;
        Ok(())
    }
    // --- Data Consumption from Subrecords ---
    /// Skips the data payload of the current subrecord.
    /// Assumes get_sub_header() was called and context.remaining_subrec_data is set.
    fn skip_sub_data_payload(&mut self) -> Result<(), EsmError> {
        let count = self.context.remaining_subrec_data;
        if count > 0 {
            self.skip_raw_bytes(count as i64,ConsumptionType::SubrecordPayload)?; 
        }
        Ok(())
    }

    /// Reads exact buffer from subrecord payload. Consumes from remaining_subrec_data & remaining_rec_data.
    /// Assumes get_sub_header() was called and context.remaining_subrec_data is set.
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
        self.read_exact(buf,ConsumptionType::SubrecordPayload)?; 
        Ok(())
    }

    /// Reads string from subrecord's data payload. Consumes all of context.remaining_subrec_data.
    /// Assumes get_sub_header() was called and context.remaining_subrec_data is set.
    pub fn get_string_view(&mut self, size_to_read: usize) -> Result<String, EsmError> {

        // Resize buffer if needed.
        if self.buffer.len() <= size_to_read {
            // Add some extra padding to reduce the chance of having to resize again later.
            self.buffer.resize(3 * size_to_read, 0);
        }
        let target_buffer_slice = &mut self.buffer[..size_to_read];

        if size_to_read > 0 {
            // Only read if there's data
            // use _get_exact_impl because otherwise we would be double borrowing self
            self.read_exact(target_buffer_slice,ConsumptionType::SubrecordPayload)?; 
        }

        let actual_str_len = target_buffer_slice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(size_to_read);
        Ok(self.text_decoder.get_utf8(&target_buffer_slice[..actual_str_len])?)
    }

    // --- High-level Subrecord Data Accessors ---
    /// Reads a string of a specified size from the current stream position.
    /// Assumes the subrecord header has already been consumed to get the size.
    pub fn get_string_with_size(&mut self, size: usize) -> Result<String, EsmError> {
        if size == 0 {
            return Ok("".to_string());
        }

        let mut buffer = vec![0u8; size];
        self.get_exact(&mut buffer, ConsumptionType::SubrecordPayload)?;

        // Morrowind strings are typically null-terminated
        // Remove the null terminator before converting to a Rust String
        if let Some(pos) = buffer.iter().position(|&x| x == 0) {
            buffer.truncate(pos);
        }

        let s = str::from_utf8(&buffer).map_err(|e| {
            EsmError::Format(format!("Failed to read string: {}", e))
        })?;
        Ok(s.to_string())
    }

    /// Assumes get_sub_name() was called 
    pub fn get_h_string(&mut self) -> Result<String, EsmError> {
        self.get_sub_header()?;
        let data_len_from_header = self.context.remaining_subrec_data;

        if self.header.format_version.0 <= MAX_STRING_REF_ID_FORMAT_VERSION_CONST.0
            && data_len_from_header == 0
                && self.context.remaining_rec_data > 0
        {
            //    The "HString MultiMark Hack" for older file versions.
            //    This addresses a quirk where an empty string subrecord (declared size 0)
            //    might be followed by a single null byte that needs to be consumed.
            let mut single_byte = [0u8];
            let current_stream_pos = self.file.stream_position()?;
            let bytes_read = match self.file.read(&mut single_byte) {
                Ok(br) => br,
                Err(ref e)
                    if e.kind() == io::ErrorKind::UnexpectedEof
                        && self.context.remaining_rec_data == 0 =>
                    {
                        0
                    }
                Err(e) => return Err(EsmError::Io(e)),
            };

            if bytes_read > 0 && single_byte[0] == 0 {
                self.context.file_pos += 1;
                self.context.remaining_rec_data -= 1;
                return Ok(String::new());
            } else if bytes_read > 0 {
                self.file.seek(SeekFrom::Start(current_stream_pos))?;
            }
        }
        self.get_string_view(self.context.remaining_subrec_data as usize)
    }
    /// This retrieves the master file index from the reader's context.
    pub fn get_master_index(&self) -> u32 {
        self.context.index as u32
    }

    /// This reads a 32-bit FormId from the stream. This is typically used for the main record header.
    pub fn get_form_id(&mut self) -> Result<u32, EsmError> {
        self.read_as(ConsumptionType::RecordContent)
    }

    pub fn get_ref_id(&mut self) -> Result<RefId, EsmError> {
        self.get_sub_header()?;
        if self.header.format_version.0 <= MAX_STRING_REF_ID_FORMAT_VERSION_CONST.0 {
            let s = self.get_string_view(self.context.remaining_subrec_data as usize)?;
            Ok(RefId::from_string_owned(s))
        } else {
            self.get_ref_id_impl()
        }
    }

    pub(crate) fn get_ref_id_impl(
        &mut self,
    ) -> Result<RefId, EsmError> {

        if self.context.remaining_subrec_data == 0 {
            return Ok(RefId::new());
        }

        let type_byte: u8 = self.read_as(ConsumptionType::RecordContent)?; 

        let result_refid = match RefIdType::try_from(type_byte)?{
            RefIdType::Empty => Ok(RefId::new()),
            RefIdType::UnsizedString => {
                let s = self.read_sub_data_string_sized(self.context.remaining_subrec_data as usize)?;
                Ok(RefId::from_string_owned(s))
            }
            RefIdType::SizedString => {
                let length_prefix_size = std::mem::size_of::<StringSizeType>();
                if self.context.remaining_subrec_data < length_prefix_size as u32 {
                    return Err(self.fail("RefId SizedString: not enough data for length prefix"));
                }
                let string_data_length: StringSizeType = self.read_as(ConsumptionType::SubrecordPayload)?;
                if self.context.remaining_subrec_data < string_data_length {
                    return Err(self.fail(&format!(
                                "RefId SizedString: declared len {} exceeds remaining payload {}",
                                string_data_length, self.context.remaining_subrec_data
                    )));
                }
                let s = self.read_sub_data_string_sized(string_data_length as usize)?;
                Ok(RefId::from_string_owned(s))
            }
            RefIdType::FormId => {
                let field_size = std::mem::size_of::<u32>() ;
                if (self.context.remaining_subrec_data as usize) < field_size * 2 {
                    return Err(self.fail("RefId FormId data too small"));
                }
                let mut form_id = FormId::default();
                form_id.index = self.read_as(ConsumptionType::SubrecordPayload)?;
                form_id.content_file_index = self.read_as(ConsumptionType::SubrecordPayload)?;
                if self.apply_content_file_mapping_internal(&mut form_id) {
                    RefId::from_form_id(form_id).map_err(EsmError::Custom)
                } else {
                    Ok(RefId::new())
                }
            }
            RefIdType::Generated => {
                let field_size = std::mem::size_of::<u64>() ;
                if (self.context.remaining_subrec_data as usize ) < field_size {
                    return Err(self.fail("RefId Generated data too small"));
                }
                let val = self.read_as(ConsumptionType::SubrecordPayload)?;
                Ok(RefId::generated(val))
            }
            RefIdType::Index => {
                let rec_type_size = Name::S_CAPACITY;
                let index_val_size = std::mem::size_of::<u32>() as usize;
                if (self.context.remaining_subrec_data as usize)< rec_type_size + index_val_size {
                    return Err(self.fail("RefId Index data too small"));
                }
                let mut rec_type_bytes = [0u8; Name::S_CAPACITY];
                self.read_sub_data_exact(&mut rec_type_bytes)?;
                let index_val= self.read_as(ConsumptionType::SubrecordPayload)?;
                Ok(RefId::index(
                        Name {
                            data: rec_type_bytes,
                        },
                        index_val,
                ))
            }
            RefIdType::ESM3ExteriorCell => {
                let field_size = std::mem::size_of::<i32>() ;
                if (self.context.remaining_subrec_data as usize)< field_size*2 {
                    return Err(self.fail("RefId ESM3ExteriorCell data too small"));
                }
                let x: i32 = self.read_as(ConsumptionType::SubrecordPayload)?;
                let y: i32 = self.read_as(ConsumptionType::SubrecordPayload)?;
                Ok(RefId::esm3_exterior_cell(x, y))
            }
        }?;

        if self.context.remaining_subrec_data > 0 {
            self.skip_sub_data()?;
        }
        Ok(result_refid)
    }


    // --- Composite Reading ---
    pub fn get_named_composite<T: EsmReadable>(&mut self, name: Name) -> Result<T, EsmError> {
        self.get_sub_name_is(name)?;
        self.get_sub_header()?;
        let result = T::read_members(self, self.context.remaining_subrec_data)?;
        if self.context.remaining_subrec_data != 0 {
            self.skip_sub_data()?;
        }
        Ok(result)
    }

    pub fn get_optional_composite<T: EsmReadable>(
        &mut self,
        name: Name,
    ) -> Result<Option<T>, EsmError> {
        if self.is_next_sub(name)? {
            self.get_sub_header()?;
            let result = T::read_members(self, self.context.remaining_subrec_data).map(Some)?;
            if self.context.remaining_subrec_data != 0 {
                self.skip_sub_data()?;
            }
            Ok(result)
        } else {
            Ok(None)
        }
    }

    pub fn get_sub_composite<T: EsmReadable>(&mut self) -> Result<T, EsmError> {
        self.get_sub_header()?;
        let result = T::read_members(self, self.context.remaining_subrec_data)?;
        if self.context.remaining_subrec_data != 0 {
            self.skip_sub_data()?;
        }
        Ok(result)
    }

    fn apply_content_file_mapping_internal(&self, id: &mut FormId) -> bool {
        if let Some(ref mapping) = self.content_file_mapping {
            if id.has_content_file() {
                if let Some(&new_file_idx) = mapping.get(&id.content_file_index) {
                    id.content_file_index = new_file_idx;
                } else {
                    return false;
                }
            }
        }
        true
    }
    pub(crate) fn fail(&self, msg: &str) -> EsmError {
        /* ... same ... */
        EsmError::Format(format!(
                "{}\n  File: {}\n  Record: {}\n  Subrecord: {}\n  Offset: 0x{:X}",
                msg,
                self.context.filename.display(),
                self.context.rec_name.to_string_lossy(),
                self.context.sub_name.to_string_lossy(),
                self.context.file_pos
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
    pub fn get_normalized_path_string(&mut self) -> Result<String, EsmError> {
        let path_str = self.get_h_string()?;
        Ok(path_str.replace('\\', "/"))
    }
}

#[derive(Debug)]
pub enum EsmError {
    Io(io::Error),
    Format(String),
    SubRecordMismatch {
        expected: Name,
        found: Name,
    },
    SubSizeMismatch {
        actual: u32,
        expected: u32,
        sub_name: Name,
        rec_name: Name,
        file: PathBuf,
        offset: u64,
    },
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
            EsmError::Custom(msg) => write!(f, "ESM Error: {}", msg),
        }
    }
}
impl std::error::Error for EsmError {}

