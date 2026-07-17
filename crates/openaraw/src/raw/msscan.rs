//! MSScan.bin index parsing.

use byteorder::{ByteOrder, LittleEndian};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SpectrumParams {
    pub format_id: u32,
    pub offset: u64,
    pub byte_count: u32,
    pub point_count: u32,
    pub uncompressed_byte_count: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ScanRecord {
    pub scan_id: u32,
    pub retention_time_min: f64,
    pub ms_level: u32,
    pub min_x: Option<f64>,
    pub max_x: Option<f64>,
    pub mrm_channel_id: Option<u32>,
    pub target_mz: Option<f64>,
    /// Precursor collision energy in eV (record offset 76, f64). Confirmed
    /// alongside `target_mz` for Auto-MS/MS records - see
    /// `docs/format/01-msscan.md`.
    pub collision_energy: Option<f64>,
    pub profile_params: Option<SpectrumParams>,
    pub centroid_params: Option<SpectrumParams>,
}

#[derive(Debug, Clone)]
pub struct MSScan {
    pub global_header_size: u32,
    pub stride: u32,
    pub records: Vec<ScanRecord>,
}

impl MSScan {
    pub fn from_path(path: &Path) -> crate::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Parse an already-loaded MSScan.bin buffer. Split out from
    /// `from_path` so tests can exercise the parser (including malformed
    /// synthetic buffers) without touching the filesystem.
    pub fn from_bytes(bytes: &[u8]) -> crate::Result<Self> {
        if bytes.len() < 92 {
            return Err(crate::Error::Parse(
                "MSScan.bin too small for header".into(),
            ));
        }

        let magic = LittleEndian::read_u32(&bytes[0..4]);
        if magic != 257 {
            return Err(crate::Error::Parse(format!(
                "Invalid MSScan magic: {}",
                magic
            )));
        }

        let global_header_size = LittleEndian::read_u32(&bytes[88..92]);
        if bytes.len() < global_header_size as usize {
            return Err(crate::Error::Parse("MSScan.bin truncated header".into()));
        }

        let payload_len = bytes.len() - global_header_size as usize;
        let possible_strides = [284, 220, 216, 196, 186];
        let mut stride = 0;

        // Need at least one 4-byte ScanID to probe strides; a payload of
        // 1..=3 bytes would otherwise slice out of bounds on a malformed file.
        if payload_len >= 4 {
            let first_scan_id = LittleEndian::read_u32(
                &bytes[global_header_size as usize..global_header_size as usize + 4],
            );
            for s in possible_strides.iter() {
                let s_val = *s as usize;
                if payload_len % s_val == 0 {
                    if payload_len == s_val {
                        stride = *s;
                        break;
                    } else if payload_len > s_val {
                        let next_scan_id = LittleEndian::read_u32(
                            &bytes[global_header_size as usize + s_val
                                ..global_header_size as usize + s_val + 4],
                        );
                        let ms_level_2 = LittleEndian::read_u16(
                            &bytes[global_header_size as usize + s_val + 20
                                ..global_header_size as usize + s_val + 22],
                        );

                        if (ms_level_2 == 1 || ms_level_2 == 2)
                            && next_scan_id > first_scan_id
                            && next_scan_id < first_scan_id + 100000
                        {
                            stride = *s;
                            break;
                        }
                    }
                }
            }
        }

        if stride == 0 {
            return Err(crate::Error::Parse(format!(
                "Cannot determine record stride for payload len {}",
                payload_len
            )));
        }

        let num_scans = payload_len / stride as usize;
        let mut records = Vec::with_capacity(num_scans);

        for i in 0..num_scans {
            let offset = global_header_size as usize + i * stride as usize;
            let record_bytes = &bytes[offset..offset + stride as usize];

            let scan_id = LittleEndian::read_u32(&record_bytes[0..4]);
            let mrm_channel_id = if stride == 186 || stride == 196 {
                Some(LittleEndian::read_u32(&record_bytes[4..8]))
            } else {
                None
            };
            let retention_time_min = LittleEndian::read_f64(&record_bytes[12..20]);
            let ms_level = LittleEndian::read_u16(&record_bytes[20..22]) as u32;

            let mut min_x = None;
            let mut max_x = None;
            if stride >= 284 {
                min_x = Some(LittleEndian::read_f64(&record_bytes[244..252]));
                max_x = Some(LittleEndian::read_f64(&record_bytes[252..260]));
            }

            // Extract SpectrumParams
            let mut profile_params = None;
            let mut centroid_params = None;

            // Known block offsets based on stride
            let block_offsets = match stride {
                186 => vec![136],
                196 => vec![144],
                216 => vec![152],
                220 => vec![156],
                284 => vec![156, 220],
                _ => vec![],
            };

            for bo in block_offsets {
                let format_id = if stride == 186 {
                    LittleEndian::read_u16(&record_bytes[bo..bo + 2]) as u32
                } else {
                    LittleEndian::read_u32(&record_bytes[bo..bo + 4])
                };

                let p_off = if stride == 186 { bo + 2 } else { bo + 4 };
                let spec_offset = LittleEndian::read_u64(&record_bytes[p_off..p_off + 8]);
                let byte_count = LittleEndian::read_u32(&record_bytes[p_off + 8..p_off + 12]);
                let point_count = LittleEndian::read_u32(&record_bytes[p_off + 12..p_off + 16]);

                // ID 1 (Profile) has uncompressed byte count
                let (unc_byte_count, _next_bo) = if format_id == 1 {
                    (
                        Some(LittleEndian::read_u32(
                            &record_bytes[p_off + 16..p_off + 20],
                        )),
                        bo + 24,
                    )
                } else {
                    (None, bo + 20)
                };

                let params = SpectrumParams {
                    format_id,
                    offset: spec_offset,
                    byte_count,
                    point_count,
                    uncompressed_byte_count: unc_byte_count,
                };

                match format_id {
                    1 => profile_params = Some(params),
                    2 | 3 => centroid_params = Some(params),
                    _ => {} // Ignore 0 or unknown
                }
            }

            // Target m/z (isolation window center) and collision energy are
            // only confirmed for Auto-MS/MS records that use the target_mz
            // field (i.e. not QQQ MRM transitions, which carry their own
            // per-channel CE elsewhere and are not covered by this offset -
            // see docs/format/01-msscan.md).
            let (target_mz, collision_energy) = if ms_level >= 2 && mrm_channel_id.is_none() {
                (
                    Some(LittleEndian::read_f64(&record_bytes[84..92])),
                    Some(LittleEndian::read_f64(&record_bytes[76..84])),
                )
            } else {
                (None, None)
            };

            records.push(ScanRecord {
                scan_id,
                retention_time_min,
                ms_level,
                min_x,
                max_x,
                mrm_channel_id,
                target_mz,
                collision_energy,
                profile_params,
                centroid_params,
            });
        }

        Ok(MSScan {
            global_header_size,
            stride,
            records,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLOBAL_HEADER_SIZE: usize = 92;

    fn header() -> Vec<u8> {
        let mut h = vec![0u8; GLOBAL_HEADER_SIZE];
        LittleEndian::write_u32(&mut h[0..4], 257); // magic
        LittleEndian::write_u32(&mut h[88..92], GLOBAL_HEADER_SIZE as u32);
        h
    }

    /// A single synthetic record of `stride` bytes with just the fields the
    /// stride-probe and top-level parse look at: scan_id, retention_time,
    /// ms_level. Everything else (block offsets, min_x/max_x, ...) is left
    /// zeroed, which the parser reads as format_id 0 ("ignore") / 0.0.
    fn record(stride: usize, scan_id: u32, ms_level: u16) -> Vec<u8> {
        let mut r = vec![0u8; stride];
        LittleEndian::write_u32(&mut r[0..4], scan_id);
        LittleEndian::write_f64(&mut r[12..20], 0.0);
        LittleEndian::write_u16(&mut r[20..22], ms_level);
        r
    }

    fn msscan_bytes(stride: u32, records: &[(u32, u16)]) -> Vec<u8> {
        let mut bytes = header();
        for &(scan_id, ms_level) in records {
            bytes.extend(record(stride as usize, scan_id, ms_level));
        }
        bytes
    }

    #[test]
    fn detects_each_candidate_stride() {
        for &stride in &[284u32, 220, 216, 196, 186] {
            let bytes = msscan_bytes(stride, &[(1, 1), (2, 1)]);
            let scan = MSScan::from_bytes(&bytes)
                .unwrap_or_else(|e| panic!("stride {stride} failed to parse: {e}"));
            assert_eq!(scan.stride, stride, "wrong stride detected for {stride}");
            assert_eq!(scan.records.len(), 2);
            assert_eq!(scan.records[0].scan_id, 1);
            assert_eq!(scan.records[1].scan_id, 2);
        }
    }

    #[test]
    fn detects_stride_from_single_exact_record() {
        // With exactly one record, the probe short-circuits on
        // payload_len == stride without needing the second-record
        // scan_id/ms_level heuristic.
        let bytes = msscan_bytes(186, &[(1, 1)]);
        let scan = MSScan::from_bytes(&bytes).unwrap();
        assert_eq!(scan.stride, 186);
        assert_eq!(scan.records.len(), 1);
    }

    /// Regression test for the bounds guard fixed alongside the stride
    /// probe: a payload of 1..=3 bytes is too short for the 4-byte ScanID
    /// the probe reads, and must produce a parse error rather than
    /// panicking on an out-of-bounds slice.
    #[test]
    fn short_payload_does_not_panic() {
        let mut bytes = header();
        bytes.extend([0u8, 0u8]); // 2-byte payload: shorter than one ScanID
        let result = MSScan::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn empty_payload_is_a_parse_error_not_a_panic() {
        let bytes = header();
        let result = MSScan::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn too_small_for_header_is_a_parse_error() {
        let result = MSScan::from_bytes(&[0u8; 10]);
        assert!(result.is_err());
    }

    #[test]
    fn bad_magic_is_a_parse_error() {
        let mut bytes = header();
        LittleEndian::write_u32(&mut bytes[0..4], 999);
        let result = MSScan::from_bytes(&bytes);
        assert!(result.is_err());
    }
}
