//! MSScan.bin index parsing.

use std::path::Path;
use byteorder::{ByteOrder, LittleEndian};

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
        if bytes.len() < 92 {
            return Err(crate::Error::Parse("MSScan.bin too small for header".into()));
        }

        let magic = LittleEndian::read_u32(&bytes[0..4]);
        if magic != 257 {
            return Err(crate::Error::Parse(format!("Invalid MSScan magic: {}", magic)));
        }

        let global_header_size = LittleEndian::read_u32(&bytes[88..92]);
        if bytes.len() < global_header_size as usize {
            return Err(crate::Error::Parse("MSScan.bin truncated header".into()));
        }

        let payload_len = bytes.len() - global_header_size as usize;
        // The stride can be inferred if we know the scan count, but we can also infer it from known instrument types.
        // Let's determine stride based on typical values or just assume it's one of the known ones.
        // Wait, is there a way to definitively know stride?
        // Let's check common strides: 186, 196, 220, 284
        // To find the exact stride, we can check payload_len % stride == 0.
        // Since payload_len is large, there's usually only one valid stride.
        let possible_strides = [284, 220, 196, 186];
        let mut stride = 0;
        for s in possible_strides.iter() {
            if payload_len % *s as usize == 0 {
                stride = *s;
                break;
            }
        }
        if stride == 0 {
            return Err(crate::Error::Parse(format!("Cannot determine record stride for payload len {}", payload_len)));
        }

        let num_scans = payload_len / stride as usize;
        let mut records = Vec::with_capacity(num_scans);

        for i in 0..num_scans {
            let offset = global_header_size as usize + i * stride as usize;
            let record_bytes = &bytes[offset..offset + stride as usize];

            let scan_id = LittleEndian::read_u32(&record_bytes[0..4]);
            let retention_time_min = LittleEndian::read_f64(&record_bytes[12..20]);
            let ms_level = LittleEndian::read_u32(&record_bytes[20..24]);

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
                220 => vec![156],
                284 => vec![156, 220],
                _ => vec![],
            };

            for bo in block_offsets {
                let format_id = if stride == 186 {
                    LittleEndian::read_u16(&record_bytes[bo..bo+2]) as u32
                } else {
                    LittleEndian::read_u32(&record_bytes[bo..bo+4])
                };

                let p_off = if stride == 186 { bo + 2 } else { bo + 4 };
                let spec_offset = LittleEndian::read_u64(&record_bytes[p_off..p_off+8]);
                let byte_count = LittleEndian::read_u32(&record_bytes[p_off+8..p_off+12]);
                let point_count = LittleEndian::read_u32(&record_bytes[p_off+12..p_off+16]);

                // ID 1 (Profile) has uncompressed byte count
                let (unc_byte_count, next_bo) = if format_id == 1 {
                    (Some(LittleEndian::read_u32(&record_bytes[p_off+16..p_off+20])), bo + 24)
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

            records.push(ScanRecord {
                scan_id,
                retention_time_min,
                ms_level,
                min_x,
                max_x,
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
