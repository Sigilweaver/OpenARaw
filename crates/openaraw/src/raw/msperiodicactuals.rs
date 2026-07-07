//! MSPeriodicActuals.bin telemetry parser.

use byteorder::{ByteOrder, LittleEndian};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ActualsRecord {
    pub retention_time_min: f64,
}

pub fn parse(path: &Path) -> crate::Result<Vec<ActualsRecord>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < 68 {
        return Ok(vec![]);
    }

    let global_header_size = LittleEndian::read_u32(&bytes[0x44..0x48]) as usize;
    if global_header_size == 0 || bytes.len() < global_header_size {
        return Ok(vec![]);
    }

    // Since actuals aren't required for spectra, we just provide a basic stub.
    Ok(vec![])
}
