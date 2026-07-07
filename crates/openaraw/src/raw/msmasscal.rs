//! MSMassCal.bin (Dynamic mass recalibration) parser.

use byteorder::{ByteOrder, LittleEndian};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct MassCalRecord {
    pub coefficients: Vec<f64>,
}

pub fn parse(path: &Path) -> crate::Result<Vec<MassCalRecord>> {
    let bytes = std::fs::read(path)?;
    let header_size = 72;
    let stride = 84;

    if bytes.len() < header_size {
        return Ok(vec![]);
    }

    let payload = &bytes[header_size..];
    let count = payload.len() / stride;
    let mut records = Vec::with_capacity(count);

    for i in 0..count {
        let offset = i * stride;
        let num_coeff = LittleEndian::read_u32(&payload[offset..offset + 4]) as usize;
        let mut coeffs = Vec::with_capacity(num_coeff);
        for j in 0..std::cmp::min(num_coeff, 10) {
            coeffs.push(LittleEndian::read_f64(
                &payload[offset + 4 + j * 8..offset + 12 + j * 8],
            ));
        }
        records.push(MassCalRecord {
            coefficients: coeffs,
        });
    }

    Ok(records)
}
