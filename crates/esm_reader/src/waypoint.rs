// src/waypoint.rs

use crate::cell::Position;
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId; // WAYP ID is a RefId // Re-use Position struct for coordinates

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Waypoint {
    pub id: RefId,                       // From NAME subrecord
    pub position_data: Option<Position>, // From DATA subrecord (Position struct)
    pub connections: Vec<u16>, // From CNAM subrecord (array of shorts - connection indices)
                               // OpenMW stores this as std::vector<int> mConnections, but file is shorts.
}

impl Waypoint {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut wayp = Waypoint::new();
        let mut id_found = false;
        let mut data_found = false; // For position data

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    wayp.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"DATA" => {
                    // Position (X, Y, Z, and 3x Rotations - though rotations often 0 for waypoints)
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<Position>() as u32; // 24 bytes
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    wayp.position_data = Some(reader.get_t::<Position>()?);
                    data_found = true;
                }
                b"CNAM" => {
                    // Connection list (array of shorts)
                    reader.get_sub_header()?;
                    let sub_size = reader.get_sub_size();
                    if sub_size % 2 != 0 {
                        // Each connection is a short (2 bytes)
                        return Err(reader.fail(&format!(
                            "WAYP record '{}': CNAM subrecord size {} is not a multiple of 2",
                            wayp.id.to_debug_string(),
                            sub_size
                        )));
                    }
                    let num_connections = sub_size / 2;
                    for _ in 0..num_connections {
                        wayp.connections.push(reader.get_t::<u16>()?);
                    }
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || wayp.id.empty() {
            return Err(reader.fail("WAYP record missing mandatory NAME subrecord for its ID"));
        }
        if !data_found {
            return Err(reader.fail(&format!(
                "WAYP record '{}' missing DATA (position) subrecord",
                wayp.id.to_debug_string()
            )));
        }
        // Connections (CNAM) can be empty if it's an isolated waypoint.

        Ok(wayp)
    }
}
