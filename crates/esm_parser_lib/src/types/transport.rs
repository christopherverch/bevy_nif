use crate::{EsmError, EsmReader, common_types::Position};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransportDestination {
    pub pos: Position,
    pub cell_name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Transport {
    pub list: Vec<TransportDestination>,
}

impl Transport {
    pub fn add(&mut self, reader: &mut EsmReader) -> Result<(), EsmError> {
        let sub_name = reader.get_sub_name();

        match sub_name.as_bytes() {
            b"DODT" => {
                reader.read_sub_header()?;
                let pos: Position = reader.load()?;
                self.list.push(TransportDestination {
                    pos,
                    cell_name: String::new(),
                });
            }
            b"DNAM" => {
                let name = reader.read_header_decode_string()?;
                if let Some(last) = self.list.last_mut() {
                    last.cell_name = name;
                } else {
                    eprintln!("Warning: Encountered DNAM record without DODT record, skipped.");
                }
            }
            _ => return Err(reader.fail("Not a valid Transport subrecord")),
        }
        Ok(())
    }
}
