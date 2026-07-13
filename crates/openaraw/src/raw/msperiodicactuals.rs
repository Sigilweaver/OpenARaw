//! MSPeriodicActuals.bin telemetry parser.
//!
//! Binary log of instrument diagnostic telemetry (pressures, temperatures,
//! voltages, charge states, etc.), sampled at irregular intervals
//! throughout the run. Present in 243/331 non-malformed corpus bundles
//! (absent for the whole PXD028295 project, a QQQ stride=186 exemplar -
//! this reader has no empirical coverage for that stride/format
//! combination). Field definitions for each channel (`ActualID`) live in
//! the companion `MSActualDefs.xml`; this parser does not read that file
//! and does not attempt to decode per-channel `Value`s (see below).
//!
//! Confirmed with zero exceptions across all 243 corpus files carrying
//! this bin: `Magic` (int32) = 265 at offset 0, a fixed 68-byte global
//! header (not a size field - see note on the old implementation below),
//! and a 20-byte record stride with a 4-byte trailer
//! (`(FileSize - 68) % 20 == 4` always). Full record layout is documented
//! in `docs/format/02-secondary-bins.md`.
//!
//! `ActualID` (record offset 4, int32) and `Time` (record offset 8,
//! double, minutes) are at the same offsets regardless of the QQQ/Q-TOF
//! record variant documented there, and are the two fields this parser
//! decodes. `Time` sequences cross-validate tightly against `MSScan.bin`'s
//! own retention-time range for the same bundle (e.g.
//! `PXD007734/160120-S1-band-1-00.d`: MSScan 0.0334-110.0038 min vs.
//! Actuals 0.0334-110.0038 min; similarly close matches in PXD011212,
//! PXD014285, PXD031771/526b_1.d, PXD012013/A1.d, PXD024074, PXD059765).
//! For QQQ bundles (PXD004747), the actuals log runs ~15 minutes past
//! MSScan.bin's own range, plausibly post-acquisition pump/column
//! telemetry rather than a parsing error.
//!
//! The `Value` field's position and type depend on both the QQQ/Q-TOF
//! record variant and the per-`ActualID` `DataType` from
//! `MSActualDefs.xml` (see `docs/format/02-secondary-bins.md`'s DataType
//! table) - decoding it reliably needs that XML cross-reference, which is
//! out of scope here. `ActualsRecord` therefore does not expose `Value`.

use byteorder::{ByteOrder, LittleEndian};
use std::path::Path;

/// Fixed size of the global header. Earlier code treated the `u32` at file
/// offset 0x44 as a dynamic `global_header_size`, but that value is always
/// `1` across the corpus - it's the leading field of the first record
/// bleeding through, not a size. The header is a constant 68 bytes.
const HEADER_SIZE: usize = 68;
const RECORD_STRIDE: usize = 20;

#[derive(Debug, Clone, PartialEq)]
pub struct ActualsRecord {
    /// Telemetry channel ID; defined per-instrument in `MSActualDefs.xml`
    /// (e.g. 27 = Gas Temp, 29 = Rough Vac, 57 = Charge State).
    pub actual_id: u32,
    pub retention_time_min: f64,
}

pub fn parse(path: &Path) -> crate::Result<Vec<ActualsRecord>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < HEADER_SIZE {
        return Ok(vec![]);
    }

    let payload = &bytes[HEADER_SIZE..];
    let count = payload.len() / RECORD_STRIDE;
    let mut records = Vec::with_capacity(count);

    for i in 0..count {
        let offset = i * RECORD_STRIDE;
        let record_bytes = &payload[offset..offset + RECORD_STRIDE];

        let actual_id = LittleEndian::read_u32(&record_bytes[4..8]);
        let retention_time_min = LittleEndian::read_f64(&record_bytes[8..16]);

        records.push(ActualsRecord {
            actual_id,
            retention_time_min,
        });
    }

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_bytes(leading: u32, actual_id: u32, time_min: f64, trailing: u32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(RECORD_STRIDE);
        buf.extend_from_slice(&leading.to_le_bytes());
        buf.extend_from_slice(&actual_id.to_le_bytes());
        buf.extend_from_slice(&time_min.to_le_bytes());
        buf.extend_from_slice(&trailing.to_le_bytes());
        buf
    }

    fn write_temp(name: &str, contents: &[u8]) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "openaraw-msperiodicactuals-test-{}-{}",
            std::process::id(),
            name
        ));
        std::fs::write(&path, contents).expect("write temp file");
        path
    }

    #[test]
    fn parses_actual_id_and_time_from_records() {
        let mut bytes = vec![0u8; HEADER_SIZE];
        bytes.extend(record_bytes(1, 27, 0.0334, 0));
        bytes.extend(record_bytes(0, 27, 5.512, 0));
        bytes.extend(record_bytes(0, 29, 110.0038, 0));
        // 4-byte trailer observed in real files, must not affect record count.
        bytes.extend_from_slice(&[0u8; 4]);

        let path = write_temp("basic.bin", &bytes);
        let records = parse(&path).expect("parse should succeed");
        let _ = std::fs::remove_file(&path);

        assert_eq!(records.len(), 3);
        assert_eq!(records[0].actual_id, 27);
        assert!((records[0].retention_time_min - 0.0334).abs() < 1e-9);
        assert_eq!(records[2].actual_id, 29);
        assert!((records[2].retention_time_min - 110.0038).abs() < 1e-9);
    }

    #[test]
    fn empty_payload_returns_no_records() {
        let bytes = vec![0u8; HEADER_SIZE];
        let path = write_temp("empty.bin", &bytes);
        let records = parse(&path).expect("parse should succeed");
        let _ = std::fs::remove_file(&path);
        assert!(records.is_empty());
    }

    #[test]
    fn file_smaller_than_header_returns_no_records() {
        let bytes = vec![0u8; HEADER_SIZE - 1];
        let path = write_temp("truncated.bin", &bytes);
        let records = parse(&path).expect("parse should succeed");
        let _ = std::fs::remove_file(&path);
        assert!(records.is_empty());
    }
}
