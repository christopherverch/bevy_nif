use crate::esm_reader::{EsmError, EsmReader, Windows1252Decoder};
use crate::refid_types::RefId;
use crate::traits::Load;
use crate::{Cell, Header};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

// This struct will store the location of the record in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordLocation {
    pub file_index: usize,
    pub offset: u64,
}

/// The top-level store that manages all loaded game data.
pub struct EsmStore {
    pub files: Vec<PathBuf>,
    pub record_index: HashMap<RefId, RecordLocation>,
    pub cell_cache: HashMap<RefId, Cell>,
}

impl EsmStore {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            record_index: HashMap::new(),
            cell_cache: HashMap::new(),
        }
    }

    /// Scans a file and builds a record index without parsing full records.
    pub fn load_index(&mut self, path: &Path) -> Result<(), EsmError> {
        let mut reader = EsmReader::new();
        // This correctly opens the file and loads the header into the reader's state.
        reader.open_raw_path(path, Some(Box::new(Windows1252Decoder)))?;

        // Now, get the already-loaded header from the reader. This is the correct
        // way to access the header that was parsed by open_raw_path.
        let header = reader.get_header();
        reader.skip_record()?;

        let file_index = self.files.len();
        self.files.push(path.to_path_buf());

        // From here, the reader is correctly positioned to begin parsing the
        // rest of the records in the file.
        while reader.has_more_recs() {
            let record_start_offset = reader.context.file_pos;

            // The `get_ref_id()` method reads the record's name and ID.
            let record_id = reader.get_ref_id()?;

            let location = RecordLocation {
                file_index,
                offset: record_start_offset,
            };
            self.record_index.insert(record_id, location);

            reader.skip_record()?;
        }
        Ok(())
    }
    /// Retrieves a cell record, either from the cache or by parsing it from the file on demand.
    pub fn get_cell(&mut self, id: &RefId) -> Result<Option<&Cell>, EsmError> {
        if self.cell_cache.contains_key(id) {
            return Ok(self.cell_cache.get(id));
        }

        if let Some(location) = self.record_index.get(id) {
            let file_path = &self.files[location.file_index];

            let mut reader = EsmReader::new();
            reader.open_raw_path(file_path, Some(Box::new(Windows1252Decoder)))?;

            reader
                .reader
                .as_mut()
                .unwrap()
                .seek(io::SeekFrom::Start(location.offset))?;
            let _rec_header = reader.get_rec_header()?;
            let cell = Cell::load(&mut reader)?;

            self.cell_cache.insert(id.clone(), cell);

            return Ok(self.cell_cache.get(id));
        }

        Ok(None)
    }
}

