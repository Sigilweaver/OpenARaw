//! MSMassCal.bin (per-scan mass calibration) parser.
//!
//! Confirmed format - see `docs/format/05-msmasscal.md` for the corpus
//! evidence (PXD001310, 13301 scans, coefficients cross-validated to full
//! double precision against `DefaultMassCal.xml`).
//!
//! [`crate::reader::Reader`] uses these records to convert Q-TOF flight
//! times to m/z (see [`crate::raw::calibration`]): record `i` belongs to
//! `MSScan.bin` record `i`.

use byteorder::{ByteOrder, LittleEndian};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct MassCalRecord {
    pub coefficients: Vec<f64>,
}

/// Size of the file header preceding the first record.
pub const HEADER_SIZE: usize = 72;
/// Size of one calibration record.
pub const RECORD_STRIDE: usize = 84;
const MAX_COEFFICIENTS: usize = (RECORD_STRIDE - 4) / 8;

pub fn parse(path: &Path) -> crate::Result<Vec<MassCalRecord>> {
    let bytes = std::fs::read(path)?;
    parse_bytes(&bytes)
}

/// Parse an already-loaded MSMassCal.bin buffer. A short file, a payload
/// that is not a whole number of records, or a record claiming more
/// coefficients than fit in it is an error rather than a silently
/// truncated result.
pub fn parse_bytes(bytes: &[u8]) -> crate::Result<Vec<MassCalRecord>> {
    if bytes.len() < HEADER_SIZE {
        return Err(crate::Error::Parse(format!(
            "MSMassCal.bin is {} bytes, shorter than its {HEADER_SIZE}-byte header",
            bytes.len()
        )));
    }

    let payload = &bytes[HEADER_SIZE..];
    if !payload.len().is_multiple_of(RECORD_STRIDE) {
        return Err(crate::Error::Parse(format!(
            "MSMassCal.bin payload of {} bytes is not a multiple of the {RECORD_STRIDE}-byte record",
            payload.len()
        )));
    }

    let count = payload.len() / RECORD_STRIDE;
    let mut records = Vec::with_capacity(count);

    for i in 0..count {
        let offset = i * RECORD_STRIDE;
        let num_coeff = LittleEndian::read_u32(&payload[offset..offset + 4]) as usize;
        if num_coeff > MAX_COEFFICIENTS {
            return Err(crate::Error::Parse(format!(
                "MSMassCal.bin record {i} claims {num_coeff} coefficients; at most {MAX_COEFFICIENTS} fit"
            )));
        }
        let coeffs = (0..num_coeff)
            .map(|j| {
                let start = offset + 4 + j * 8;
                LittleEndian::read_f64(&payload[start..start + 8])
            })
            .collect();
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
    fn file_smaller_than_header_is_an_error() {
        assert!(parse_bytes(&[0u8; 71]).is_err());
    }

    #[test]
    fn header_only_file_has_no_records() {
        assert!(parse_bytes(&[0u8; 72]).unwrap().is_empty());
    }

    #[test]
    fn partial_trailing_record_is_an_error() {
        let mut bytes = vec![0u8; 72];
        bytes.extend(record_bytes(&[1.0, 2.0]));
        bytes.extend([0u8; 10]);
        let err = parse_bytes(&bytes).unwrap_err();
        assert!(err.to_string().contains("not a multiple"));
    }

    #[test]
    fn oversized_coefficient_count_is_an_error() {
        let mut bytes = vec![0u8; 72];
        let mut rec = record_bytes(&[1.0, 2.0]);
        rec[0..4].copy_from_slice(&11u32.to_le_bytes());
        bytes.extend(rec);
        let err = parse_bytes(&bytes).unwrap_err();
        assert!(err.to_string().contains("claims 11 coefficients"));
    }
}
