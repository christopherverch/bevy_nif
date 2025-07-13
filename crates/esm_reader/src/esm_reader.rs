#![allow(dead_code)]
// esreader.rs (Rewritten with borrow checker fixes)

use crate::common_types::{
    EsmContext, FormatVersion, MAX_STRING_REF_ID_FORMAT_VERSION_CONST, Name, StringSizeType,
};
use crate::header_types::Header;
use crate::refid_types::{FormId, RefId, RefIdType};
use crate::{Name32, Name64};

use encoding_rs;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
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
    fn get_utf8<'a>(&self, input: &'a [u8]) -> Result<Cow<'a, str>, EsmError>;
}
pub struct SimpleTextDecoder;
impl TextDecoder for SimpleTextDecoder {
    fn get_utf8<'a>(&self, input: &'a [u8]) -> Result<Cow<'a, str>, EsmError> {
        Ok(String::from_utf8_lossy(input))
    }
}
#[derive(Debug, Default, Clone, Copy)]
pub struct Windows1252Decoder;

impl TextDecoder for Windows1252Decoder {
    fn get_utf8<'a>(&self, input: &'a [u8]) -> Result<Cow<'a, str>, EsmError> {
        let (decoded_cow, _encoding_used, _had_errors) = encoding_rs::WINDOWS_1252.decode(input);
        Ok(decoded_cow)
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

pub struct EsmReader<'a> {
    pub(crate) reader: Option<Box<dyn ReadSeek + 'a>>,
    pub(crate) context: EsmContext,
    pub(crate) record_flags: u32,
    pub(crate) buffer: Vec<u8>,
    pub(crate) text_decoder: Option<Box<dyn TextDecoder>>,
    pub(crate) file_size: u64,
    pub(crate) header: Header,
    pub(crate) content_file_mapping: Option<HashMap<u32, u32>>,
}

impl<'a> Default for EsmReader<'a> {
    fn default() -> Self {
        EsmReader {
            reader: None,
            context: EsmContext::default(),
            record_flags: 0,
            buffer: Vec::with_capacity(50 * 1024),
            text_decoder: None,
            file_size: 0,
            header: Header::default(),
            content_file_mapping: None,
        }
    }
}
pub trait EsmReadable: Sized + Default {
    /// Loads the fields of this struct from the reader.
    /// The reader is assumed to be positioned at the start of the subrecord's data,
    /// after its name and size have been read by `get_sub_header`.
    /// The `expected_size` is the size reported by the subrecord header.
    fn read_members(reader: &mut EsmReader, expected_size: u32) -> Result<Self, EsmError>;
}
macro_rules! get_reader_or_return {
    // $self will be the `self` instance
    // $err_msg_context will be a string literal like "get_exact"
    ($self:ident, $err_msg_context:literal) => {
        match &mut $self.reader {
            Some(boxed_reader_ref) => boxed_reader_ref, // Returns &mut Box<dyn ReadSeek + 'a>
            None => {
                return Err($self.fail(concat!("Reader not open for ", $err_msg_context)));
            }
        }
    };
}
impl<'a> EsmReader<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_header(&self) -> &Header {
        &self.header
    }
    pub fn get_format_version(&self) -> FormatVersion {
        self.header.format_version
    }
    pub fn ret_sub_name(&self) -> Name {
        self.context.sub_name
    }
    pub fn get_sub_size(&self) -> u32 {
        self.context.remaining_subrec_data
    }
    pub fn get_record_flags(&self) -> u32 {
        self.record_flags
    }
    pub fn get_index(&self) -> usize {
        self.context.index
    }
    pub fn set_index(&mut self, index: usize) {
        self.context.index = index;
    }

    pub fn get_file_offset(&mut self) -> Result<u64, EsmError> {
        if self.reader.is_none() {
            return Err(self.fail("Reader not open for get_file_offset"));
        }
        self.reader
            .as_mut()
            .unwrap()
            .stream_position()
            .map_err(EsmError::from)
    }

    pub fn get_context_for_deferred_load(&mut self) -> Result<EsmContext, EsmError> {
        if let Some(ref mut reader_instance) = self.reader {
            self.context.file_pos = reader_instance.stream_position()?;
        }
        Ok(self.context.clone())
    }

    pub fn restore_context(&mut self, rc: &EsmContext) -> Result<(), EsmError> {
        if !rc.filename.as_os_str().is_empty()
            && (self.context.filename != rc.filename || self.reader.is_none())
        {
            let default_decoder =
                Some(Box::new(Windows1252Decoder::default()) as Box<dyn TextDecoder>);
            self.open_raw_path(rc.filename.as_path(), default_decoder)?;
        }

        let original_reader_state = self.reader.take();
        self.context = rc.clone();
        self.reader = original_reader_state;

        if let Some(ref mut reader_instance) = self.reader {
            if reader_instance.stream_position()? != self.context.file_pos {
                reader_instance.seek(SeekFrom::Start(self.context.file_pos))?;
            }
        }
        Ok(())
    }

    pub fn close(&mut self) {
        self.reader = None;
        self.context.clear();
        self.header.blank();
        self.file_size = 0;
    }
    fn perform_shared_open_logic(
        &mut self,
        // Note: self.reader is assumed to be already set by the caller
        path_for_context: &Path,
        file_size: u64,
        text_decoder: Option<Box<dyn TextDecoder>>,
    ) -> Result<(), EsmError> {
        // Assign common context fields
        self.context.filename = path_for_context.to_path_buf();
        self.file_size = file_size;
        self.context.remaining_file_data = self.file_size; // Usually same as file_size on init
        self.text_decoder = text_decoder;

        if let Some(ref mut reader_instance) = self.reader {
            self.context.file_pos = reader_instance.seek(SeekFrom::Start(0))?;
        } else {
            // This should ideally not be hit if called after self.reader is set.
            return Err(self.fail("Reader not initialized before shared open logic"));
        }
        // Common header parsing logic
        if self.get_rec_name()? != Name::new("TES3") {
            println!("self_record {}",self.context.rec_name.to_string_view());
            return Err(self.fail("Not a valid Morrowind file (must start with TES3)"));
        }
        let flags = self.get_rec_header_flags_internal_and_return()?;
        self.record_flags = flags;
        self.header = Header::from_reader(self)?;
        Ok(())
    }
    pub fn open_raw_path(
        &mut self,
        filename: &Path,
        text_decoder: Option<Box<dyn TextDecoder>>,
    ) -> Result<(), EsmError> {
        let file = File::open(filename)?;
        let metadata = file.metadata()?;
        let actual_file_size = metadata.len();
        self.close(); // Reset EsmReader state
        self.reader = Some(Box::new(BufReader::new(file)));
        self.perform_shared_open_logic(filename, actual_file_size, text_decoder)
    }

    pub fn open_from_buffer(
        &mut self,
        file_path_for_context: &Path,
        bytes: &'a [u8],
        text_decoder: Option<Box<dyn TextDecoder>>,
    ) -> Result<(), EsmError> {
        let buffer_file_size = bytes.len() as u64;
        self.close();
        let cursor = Cursor::new(bytes);
        self.reader = Some(Box::new(cursor));
        self.perform_shared_open_logic(file_path_for_context, buffer_file_size, text_decoder)
    }
    //only use this for testing
    pub fn test_esmreader_new_for_single_record_from_stream(
        stream: Box<dyn ReadSeek + 'a>,
        file_path_for_context: &Path,
        text_decoder: Option<Box<dyn TextDecoder>>,
        main_file_header: Header,
        record_full_size_including_type_and_header: u64,
    ) -> Result<Self, EsmError> {
        let mut reader = Self {
            reader: Some(stream),
            context: EsmContext {
                filename: file_path_for_context.to_path_buf(),
                remaining_file_data: record_full_size_including_type_and_header,
                remaining_rec_data: 0,
                remaining_subrec_data: 0,
                rec_name: Name::default(),
                sub_name: Name::default(),
                index: main_file_header.data.num_records as usize,
                parent_file_indices: Vec::new(),
                sub_cached: false,
                file_pos: 0,
            },
            header: main_file_header,
            record_flags: 0,
            buffer: Vec::with_capacity(8192),
            text_decoder,
            file_size: record_full_size_including_type_and_header as u64,
            content_file_mapping: None,
        };
        if let Some(ref mut r) = reader.reader {
            reader.context.file_pos = r.stream_position()?;
        }
        Ok(reader)
    }

    // --- Low-level I/O ---
    //this exists so we can easily just pass a reader instead of a ton of args
    /// Reads exactly `buf.len()` bytes into `buf`. Updates `context.file_pos`.
    pub(crate) fn get_exact(&mut self, buf: &mut [u8], consumption_type:ConsumptionType) -> Result<(), EsmError> {
        let len_to_read_i64 = buf.len() as i64; 
        let len_to_read_u32 = buf.len() as u32;
        match consumption_type {
            ConsumptionType::RecordContent => {
                if self.context.remaining_rec_data < len_to_read_i64 {
                    return Err(self.fail(&format!(
                    "EOF (record budget): Cannot read {} bytes for {:?}. Only {} bytes left in record. File pos: 0x{:X}",
                    buf.len(), consumption_type, self.context.remaining_rec_data, self.context.file_pos
                )));
            }
        }
        ConsumptionType::SubrecordPayload => {
            if self.context.remaining_rec_data < len_to_read_i64 {
                return Err(self.fail(&format!(
                    "EOF (record budget for subrec): Cannot read {} bytes for {:?}. Only {} bytes left in record. File pos: 0x{:X}",
                    buf.len(), consumption_type, self.context.remaining_rec_data, self.context.file_pos
                )));
            }
            if self.context.remaining_subrec_data < len_to_read_u32 {
                return Err(self.fail(&format!(
                    "EOF (subrecord budget): Cannot read {} bytes for {:?}. Only {} bytes left in subrecord. File pos: 0x{:X}",
                    buf.len(), consumption_type, self.context.remaining_subrec_data, self.context.file_pos
                )));
            }
        }
        ConsumptionType::FileOnly => {
            // No additional budget checks here; _get_exact_impl handles remaining_file_data.
        }
    }
        EsmReader::_get_exact_impl(
            &mut self.reader,
            &mut self.context.file_pos,
            &mut self.context.remaining_file_data,
            &mut self.context.remaining_rec_data,
            &mut self.context.remaining_subrec_data,
            buf,
            consumption_type
        )
    }
    fn _get_exact_impl(
        reader_opt: &mut Option<Box<dyn ReadSeek + 'a>>,
        context_file_pos: &mut u64,
        context_remaining_file_data: &mut u64,
        context_remaining_rec_data: &mut i64,
        context_remaining_subrec_data: &mut u32,
        buf_to_fill: &mut [u8],
        consumption_type:ConsumptionType
    ) -> Result<(), EsmError> {

        let len_to_read = buf_to_fill.len() as u64;
        if *context_remaining_file_data < len_to_read {
            return Err(EsmError::Custom(format!(
                        // Adjust to your actual EsmError construction
                        "EOF: Not enough data for \"_get_exact_impl\" (need {}, have {}). File pos: 0x{:X}",
                        len_to_read, *context_remaining_file_data, *context_file_pos
            )));
        }
        let reader_instance = match reader_opt.as_mut() {
            Some(r) => r,
            None => {
                return Err(EsmError::Custom(
                        "Reader not open for \"_get_exact_impl\"".to_string(),
                ));
            }
        };
        // Perform the actual read from the stream
        reader_instance.read_exact(buf_to_fill).map_err(|io_err| {
            EsmError::Custom(format!(
                    "IO error during \"_get_exact_impl\" (tried to read {} bytes at pos 0x{:X}): {}",
                    len_to_read, *context_file_pos, io_err
            ))
        })?;

        match consumption_type {
        ConsumptionType::RecordContent => {
            *context_remaining_rec_data -= buf_to_fill.len() as i64;
        }
        ConsumptionType::SubrecordPayload => {
            *context_remaining_rec_data -= buf_to_fill.len() as i64;
           *context_remaining_subrec_data -= buf_to_fill.len() as u32;
        }
        ConsumptionType::FileOnly => {
            // No additional record/subrecord counters to update here
        }
    }
        *context_file_pos += len_to_read;
        *context_remaining_file_data -= len_to_read;
        Ok(())
    }
    fn _read_and_convert_le<T, const LEN: usize>(
        &mut self,
        from_le_converter: fn([u8; LEN]) -> T,
        consumption_type: ConsumptionType,
    ) -> Result<T, EsmError> {
        let mut buffer = [0u8; LEN]; // Array of known size LEN
        self.get_exact(&mut buffer,consumption_type)?;
        Ok(from_le_converter(buffer))
        }

    /// Updates `context.file_pos`.
    pub(crate) fn get_u32_le(&mut self, consumption_type:ConsumptionType) -> Result<u32, EsmError> {
        self._read_and_convert_le(u32::from_le_bytes,consumption_type)
    }
    pub(crate) fn get_i16_le(&mut self, consumption_type:ConsumptionType) -> Result<i16, EsmError> {
        self._read_and_convert_le(i16::from_le_bytes,consumption_type)
    }
    pub(crate) fn get_u64_le(&mut self, consumption_type:ConsumptionType) -> Result<u64, EsmError> {
        self._read_and_convert_le(u64::from_le_bytes,consumption_type)
    }

    pub(crate) fn get_i32_le(&mut self, consumption_type:ConsumptionType) -> Result<i32, EsmError> {
        self._read_and_convert_le(i32::from_le_bytes,consumption_type)
    }

    pub(crate) fn get_f32_le(&mut self, consumption_type:ConsumptionType) -> Result<f32, EsmError> {
        self._read_and_convert_le(f32::from_le_bytes,consumption_type)
    }

    pub(crate) fn get_name32_le(&mut self, consumption_type:ConsumptionType) -> Result<Name32, EsmError> {
        self._read_and_convert_le(Name32::from_le_bytes,consumption_type)
    }

    pub(crate) fn get_name64_le(&mut self, consumption_type:ConsumptionType) -> Result<Name64, EsmError> {
        self._read_and_convert_le(Name64::from_le_bytes,consumption_type)
    }

    pub(crate) fn get_name_le(&mut self, consumption_type:ConsumptionType) -> Result<Name, EsmError> {
        self._read_and_convert_le(Name::from_le_bytes,consumption_type)
    }

    pub(crate) fn get_u8_le(&mut self, consumption_type:ConsumptionType) -> Result<u8, EsmError> {
        self._read_and_convert_le(u8::from_le_bytes,consumption_type)
    }
    pub(crate) fn get_u16_le(&mut self, consumption_type:ConsumptionType) -> Result<u16, EsmError> {
        self._read_and_convert_le(u16::from_le_bytes,consumption_type)
    }
    pub(crate) fn get_i8_le(&mut self, consumption_type:ConsumptionType) -> Result<i8, EsmError> {
        self._read_and_convert_le(i8::from_le_bytes,consumption_type)
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
        let abs_offset_for_rec_data = offset.abs(); 
        let abs_offset_for_subrec_data = offset.abs() as u32; 

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
                    if self.context.remaining_rec_data < abs_offset_for_rec_data {
                        return Err(self.fail(&format!(
                                    "EOF (record budget): Cannot skip forward {} bytes (have {}). Context: {:?}, File pos: 0x{:X}",
                                    offset, self.context.remaining_rec_data, consumption_type, self.context.file_pos
                        )));
                    }
                }
                ConsumptionType::SubrecordPayload => {
                    if self.context.remaining_rec_data < abs_offset_for_rec_data {
                        return Err(self.fail(&format!("EOF (record for subrec): Cannot skip forward {} bytes (have {}). Context: {:?}, File pos: 0x{:X}", offset, self.context.remaining_rec_data, consumption_type, self.context.file_pos)));
                    }
                    if self.context.remaining_subrec_data < abs_offset_for_subrec_data {
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
        let reader_instance = get_reader_or_return!(self, "skip_raw_bytes");
        let original_tracked_pos = self.context.file_pos;
        let new_stream_pos = reader_instance.seek(SeekFrom::Current(offset)).map_err(|e| {
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
                    self.context.remaining_rec_data -= abs_offset_for_rec_data;
                }
                ConsumptionType::SubrecordPayload => {
                    self.context.remaining_rec_data -= abs_offset_for_rec_data;
                    self.context.remaining_subrec_data -= abs_offset_for_subrec_data;
                }
                ConsumptionType::FileOnly => {}
            }
        } else { // offset < 0 (Backward skip)
            self.context.remaining_file_data += abs_offset_as_u64; // Add back to remaining file data
            match consumption_type {
                ConsumptionType::RecordContent => {
                    self.context.remaining_rec_data += abs_offset_for_rec_data; // Add back
                }
                ConsumptionType::SubrecordPayload => {
                    self.context.remaining_rec_data += abs_offset_for_rec_data; // Add back
                    self.context.remaining_subrec_data += abs_offset_for_subrec_data; // Add back
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
            self.skip_raw_bytes(self.context.remaining_rec_data, ConsumptionType::RecordContent)?
        }

        let mut name_bytes = [0u8; Name::S_CAPACITY];
        self.get_exact(&mut name_bytes, ConsumptionType::FileOnly)?;
        self.context.rec_name = Name { data: name_bytes };
        self.context.sub_cached = false;
        Ok(self.context.rec_name)
    }

    fn get_rec_header_flags_internal_and_return(&mut self) -> Result<u32, EsmError> {
        if self.context.remaining_file_data < 12 {
            return Err(self.fail("EOF while reading record header"));
        }
        let data_size = self.get_u32_le(ConsumptionType::FileOnly)?;
        let _unused_header = self.get_u32_le(ConsumptionType::FileOnly)?;
        let flags_val = self.get_u32_le(ConsumptionType::FileOnly)?;

        self.context.remaining_rec_data = data_size as i64; // This is the size of the data block *following* this header

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
        self.record_flags = flags;
        Ok(flags)
    }

    pub fn skip_record(&mut self) -> Result<(), EsmError> {
        let bytes_to_skip_in_data_block = self.context.remaining_rec_data;
        self.skip_raw_bytes(bytes_to_skip_in_data_block,ConsumptionType::RecordContent)?;
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
        self.get_exact(&mut name_bytes,ConsumptionType::RecordContent)?;
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
        let sub_data_payload_size = self.get_u32_le(ConsumptionType::RecordContent)?;
        self.context.remaining_subrec_data = sub_data_payload_size;


        // Validate the declared subrecord data payload size against what's *truly* left
        if (self.context.remaining_subrec_data as i64) > self.context.remaining_rec_data {
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
        self.get_exact(buf,ConsumptionType::SubrecordPayload)?; 
        Ok(())
    }

    /// Reads string from subrecord's data payload. Consumes all of context.remaining_subrec_data.
    /// Assumes get_sub_header() was called and context.remaining_subrec_data is set.
    fn read_sub_data_string_payload(&mut self) -> Result<String, EsmError> {
        let size_to_read = self.context.remaining_subrec_data as usize;

        // Resize buffer if needed.
        if self.buffer.len() < size_to_read {
            self.buffer
                .resize(size_to_read.max(self.buffer.capacity().max(512)), 0);
        }
        let target_buffer_slice = &mut self.buffer[..size_to_read];

        if size_to_read > 0 {
            // Only read if there's data
            // use _get_exact_impl because otherwise we would be double borrowing self
            EsmReader::_get_exact_impl(
                &mut self.reader,
                &mut self.context.file_pos,
                &mut self.context.remaining_file_data,
                &mut self.context.remaining_rec_data,
                &mut self.context.remaining_subrec_data,
                target_buffer_slice,
                ConsumptionType::SubrecordPayload
            )?; // Updates file_pos
        }

        let actual_str_len = target_buffer_slice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(size_to_read);
        if let Some(ref text_decoder) = self.text_decoder {
            Ok(text_decoder
                .get_utf8(&target_buffer_slice[..actual_str_len])?
                .into_owned())
        } else {
            Ok(String::from_utf8_lossy(&target_buffer_slice[..actual_str_len]).into_owned())
        }
    }

    // --- High-level Subrecord Data Accessors ---
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
            let reader_instance = get_reader_or_return!(self, "get_h_string_multimark_hack");
            let mut single_byte = [0u8];
            let current_stream_pos = reader_instance.stream_position()?;
            let bytes_read = match reader_instance.read(&mut single_byte) {
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
                reader_instance.seek(SeekFrom::Start(current_stream_pos))?;
            }
        }
        self.read_sub_data_string_payload()
    }

    pub fn get_ref_id(&mut self) -> Result<RefId, EsmError> {
        self.get_sub_header()?;
        if self.header.format_version.0 <= MAX_STRING_REF_ID_FORMAT_VERSION_CONST.0 {
            let s = self.read_sub_data_string_payload()?;
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

        let type_byte: u8 = self.get_u8_le(ConsumptionType::RecordContent)?; 

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
                let string_data_length: StringSizeType = self.get_u32_le(ConsumptionType::SubrecordPayload)?;
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
                form_id.index = self.get_u32_le(ConsumptionType::SubrecordPayload)?;
                form_id.content_file_index = self.get_u32_le(ConsumptionType::SubrecordPayload)?;
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
                let val = self.get_u64_le(ConsumptionType::SubrecordPayload)?;
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
                let index_val= self.get_u32_le(ConsumptionType::SubrecordPayload)?;
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
                let x: i32 = self.get_i32_le(ConsumptionType::SubrecordPayload)?;
                let y: i32 = self.get_i32_le(ConsumptionType::SubrecordPayload)?;
                Ok(RefId::esm3_exterior_cell(x, y))
            }
        }?;

        if self.context.remaining_subrec_data > 0 {
            self.skip_sub_data()?;
        }
        Ok(result_refid)
    }

    fn read_sub_data_string_sized(&mut self, size_to_read: usize) -> Result<String, EsmError> {
        if size_to_read == 0 {
            return Ok(String::new());
        }
        if size_to_read > self.context.remaining_subrec_data as usize {
            return Err(self.fail(&format!("read_sub_data_string_sized: Attempty to read {} bytes from subrecord payload with {} bytes left", size_to_read, self.context.remaining_subrec_data)));
        }

        if self.buffer.len() < size_to_read {
            self.buffer.resize(size_to_read.max(512), 0);
        }

        let target_buffer_slice = &mut self.buffer[..size_to_read];
        EsmReader::_get_exact_impl(&mut self.reader, &mut self.context.file_pos, &mut self.context.remaining_file_data,&mut self.context.remaining_rec_data, &mut self.context.remaining_subrec_data,  target_buffer_slice,ConsumptionType::SubrecordPayload )?; 
        let actual_str_len = target_buffer_slice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(size_to_read);
        if let Some(ref text_decoder) = self.text_decoder {
            Ok(
                text_decoder
                .get_utf8(&target_buffer_slice[..actual_str_len])?
                .into_owned())
        } else {
            Ok(String::from_utf8_lossy(&target_buffer_slice[..actual_str_len]).into_owned())
        }
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
    pub(crate) fn report_sub_size_mismatch(&self, expected: u32, actual: u32) -> EsmError {
        /* ... same ... */
        EsmError::SubSizeMismatch {
            actual,
            expected,
            sub_name: self.context.sub_name,
            rec_name: self.context.rec_name,
            file: self.context.filename.clone(),
            offset: self.context.file_pos,
        }
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
