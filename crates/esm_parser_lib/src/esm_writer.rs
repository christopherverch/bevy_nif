use bytemuck::Pod;
use to_utf8::Utf8Encoder;

use crate::{EsmError, Header, Name4, RefId, RefIdType, StringSizeType};

use std::io::{BufWriter, Seek, SeekFrom, Write};

pub trait WriteSeek: Write + Seek + Send + Sync {}
impl<T: Write + Seek + Send + Sync> WriteSeek for T {}

pub struct RecordData {
    name: Name4,
    position: u64,
    size: u32,
}

pub struct EsmWriter {
    pub(crate) stream: BufWriter<Box<dyn WriteSeek>>,
    pub(crate) records: Vec<RecordData>,
    pub(crate) text_decoder: Utf8Encoder,
    pub(crate) header: Header,
    pub(crate) record_count: i32,
    pub(crate) counting: bool,
}

impl EsmWriter {
    pub fn new(text_decoder: Utf8Encoder, stream: Box<dyn WriteSeek>) -> Self {
        Self {
            stream: BufWriter::new(stream),
            records: Vec::new(),
            text_decoder,
            header: Header::default(),
            record_count: 0,
            counting: true,
        }
    }

    pub fn set_header(&mut self, header: Header) {
        self.header = header;
    }

    pub fn set_encoder(&mut self, encoder: Utf8Encoder) {
        self.text_decoder = encoder;
    }

    pub fn get_record_count(&self) -> i32 {
        self.record_count
    }

    pub fn start_record(&mut self, name: Name4, flags: u32) -> Result<(), EsmError> {
        self.record_count += 1;
        self.write_name(name)?;

        let position = self.stream.stream_position()?;
        self.write_t(0u32)?; // Size placeholder
        self.write_t(0u32)?; // Unused header data
        self.write_t(flags)?;

        self.records.push(RecordData {
            name,
            position,
            size: 0,
        });
        Ok(())
    }

    pub fn start_sub_record(&mut self, name: Name4) -> Result<(), EsmError> {
        self.write_name(name)?;
        let position = self.stream.stream_position()?;
        self.write_t(0u32)?; // Subrecord size placeholder

        self.records.push(RecordData {
            name,
            position,
            size: 0,
        });
        Ok(())
    }

    pub fn end_record(&mut self, name: Name4) -> Result<(), EsmError> {
        let rec = self.records.pop().ok_or_else(|| {
            EsmError::Custom("Unbalanced end_record: no active record/subrecord".to_string())
        })?;

        if rec.name != name {
            return Err(EsmError::Custom(format!(
                "Mismatched end_record: expected '{}', found '{}'",
                name.to_string_lossy(),
                rec.name.to_string_lossy()
            )));
        }

        let current_pos = self.stream.stream_position()?;
        self.stream.seek(SeekFrom::Start(rec.position))?;

        self.counting = false;
        self.write(&rec.size.to_le_bytes())?;
        self.counting = true;

        self.stream.seek(SeekFrom::Start(current_pos))?;
        Ok(())
    }

    pub fn write_header_string(&mut self, name: Name4, data: &str) -> Result<(), EsmError> {
        self.start_sub_record(name)?;
        self.write_string(data)?;
        self.end_record(name)
    }

    pub fn write_header_string_padded(
        &mut self,
        name: Name4,
        data: &str,
        size: usize,
    ) -> Result<(), EsmError> {
        if data.len() > size {
            return Err(EsmError::Custom(format!(
                "String data too long for padded write: {} > {}",
                data.len(),
                size
            )));
        }
        self.start_sub_record(name)?;
        self.write_string(data)?;
        if data.len() < size {
            let padding = vec![0u8; size - data.len()];
            self.write(&padding)?;
        }
        self.end_record(name)
    }

    pub fn write_header_c_string(&mut self, name: Name4, data: &str) -> Result<(), EsmError> {
        self.start_sub_record(name)?;
        self.write_string(data)?;
        if !data.ends_with('\0') {
            self.write(b"\0")?;
        }
        self.end_record(name)
    }

    pub fn write_header_string_opt(&mut self, name: Name4, data: &str) -> Result<(), EsmError> {
        if !data.is_empty() {
            self.write_header_string(name, data)?;
        }
        Ok(())
    }

    pub fn write_header_c_string_opt(&mut self, name: Name4, data: &str) -> Result<(), EsmError> {
        if !data.is_empty() {
            self.write_header_c_string(name, data)?;
        }
        Ok(())
    }

    pub fn write_header_refid(&mut self, name: Name4, value: RefId) -> Result<(), EsmError> {
        self.start_sub_record(name)?;
        self.write_refid(&value)?;
        self.end_record(name)
    }

    pub fn write_header_c_refid(&mut self, name: Name4, value: RefId) -> Result<(), EsmError> {
        self.start_sub_record(name)?;
        self.write_refid(&value)?;
        self.end_record(name)
    }

    pub fn write_header_refid_opt(&mut self, name: Name4, value: RefId) -> Result<(), EsmError> {
        if !value.is_empty() {
            self.write_header_refid(name, value)?;
        }
        Ok(())
    }

    pub fn write_header_c_refid_opt(&mut self, name: Name4, value: RefId) -> Result<(), EsmError> {
        if !value.is_empty() {
            self.write_header_c_refid(name, value)?;
        }
        Ok(())
    }

    pub fn write_string(&mut self, data: &str) -> Result<(), EsmError> {
        if data.is_empty() {
            self.write(b"\0")
        } else {
            let encoded = self.text_decoder.get_legacy_enc(data).into_owned();
            self.write(&encoded)
        }
    }

    pub fn write_c_string(&mut self, data: &str) -> Result<(), EsmError> {
        self.write_string(data)?;
        if !data.ends_with('\0') {
            self.write(b"\0")?;
        }
        Ok(())
    }

    pub fn write_refid(&mut self, value: &RefId) -> Result<(), EsmError> {
        match value {
            RefId::Empty(_) => {
                self.write_t(RefIdType::Empty as u8)?;
            }
            RefId::UnsizedString(s) => {
                self.write_t(RefIdType::UnsizedString as u8)?;
                self.write(s.0.as_bytes())?;
            }
            RefId::SizedString(s) => {
                self.write_t(RefIdType::SizedString as u8)?;
                let len = s.0.len();
                let stored_size: StringSizeType = len.try_into().map_err(|_| {
                    EsmError::Custom("RefId string size exceeds StringSizeType".to_string())
                })?;
                self.write_t(stored_size)?;
                self.write(s.0.as_bytes())?;
            }
            RefId::FormId(f) => {
                self.write_t(RefIdType::FormId as u8)?;
                self.write_t(f.to_u32()?)?;
            }
            RefId::Generated(g) => {
                self.write_t(RefIdType::Generated as u8)?;
                self.write_t(g.value)?;
            }
            RefId::Index(i) => {
                self.write_t(RefIdType::Index as u8)?;
                self.write_t(i.record_type as u32)?;
                self.write_t(i.value)?;
            }
            RefId::ESM3ExteriorCell(c) => {
                self.write_t(RefIdType::ESM3ExteriorCell as u8)?;
                self.write_t(c.x)?;
                self.write_t(c.y)?;
            }
        }
        Ok(())
    }

    pub fn write_name(&mut self, name: Name4) -> Result<(), EsmError> {
        self.write(name.as_bytes())
    }

    pub fn write_typed_header<T: Pod>(&mut self, name: Name4, data: &T) -> Result<(), EsmError> {
        self.start_sub_record(name)?;
        self.write_t(*data)?;
        self.end_record(name)
    }

    pub fn write_t<T: Pod>(&mut self, data: T) -> Result<(), EsmError> {
        self.write(bytemuck::bytes_of(&data))
    }

    pub fn write(&mut self, data: &[u8]) -> Result<(), EsmError> {
        if self.counting && !self.records.is_empty() {
            for rec in &mut self.records {
                rec.size += data.len() as u32;
            }
        }
        self.stream.write_all(data)?;
        Ok(())
    }

    pub fn write_typed(&self, _count: i32) {}
}
