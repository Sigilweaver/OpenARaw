//! MSMassCal.bin (Dynamic mass recalibration) parser.
//!
//! Confirmed format - see `docs/format/05-msmasscal.md` for the full
//! corpus evidence (PXD001310, 13301 scans, coefficients cross-validated
//! to full double precision against `DefaultMassCal.xml`).
//!
//! `parse` is not called by [`crate::reader::Reader`]: per the "Calibration
//! Application Note" in that doc, these coefficients are already baked
//! into the centroid data in `MSPeak.bin` by the acquisition firmware, so
//! they are not needed to reproduce spectra faithfully. `openmassspec_core`'s
//! output schema (`SpectrumRecord`/`RunMetadata`) also has no field for
//! per-scan calibration provenance metadata like this. The function is
//! kept `pub` as a deliberate raw-level API - useful to callers of this
//! crate directly (e.g. QC/provenance tooling) that want the calibration
//! coefficients even though they don't flow through the `SpectrumSource`
//! pipeline - rather than removed, since the format is fully confirmed and
//! the parser is correct.

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

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp(name: &str, contents: &[u8]) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "openaraw-msmasscal-test-{}-{}",
            std::process::id(),
            name
        ));
        std::fs::write(&path, contents).expect("write temp file");
        path
    }

    fn record_bytes(coeffs: &[f64]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(84);
        buf.extend_from_slice(&(coeffs.len() as u32).to_le_bytes());
        for c in coeffs {
            buf.extend_from_slice(&c.to_le_bytes());
        }
        buf.resize(84, 0);
        buf
    }

    #[test]
    fn parses_coefficients_per_record() {
        let mut bytes = vec![0u8; 72];
        let coeffs_a = [0.0003479961, 12.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let coeffs_b = [0.0003479973, 12.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        bytes.extend(record_bytes(&coeffs_a));
        bytes.extend(record_bytes(&coeffs_b));

        let path = write_temp("basic.bin", &bytes);
        let records = parse(&path).expect("parse should succeed");
        let _ = std::fs::remove_file(&path);

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].coefficients.len(), 10);
        assert!((records[0].coefficients[0] - 0.0003479961).abs() < 1e-12);
        assert!((records[1].coefficients[0] - 0.0003479973).abs() < 1e-12);
    }

    #[test]
    fn file_smaller_than_header_returns_no_records() {
        let bytes = vec![0u8; 71];
        let path = write_temp("truncated.bin", &bytes);
        let records = parse(&path).expect("parse should succeed");
        let _ = std::fs::remove_file(&path);
        assert!(records.is_empty());
    }
}
