//! MSPeak.bin (Centroid peak data) parser.

use crate::raw::msscan::SpectrumParams;
use byteorder::{ByteOrder, LittleEndian};

#[derive(Debug, Clone)]
pub struct PeakSpectrum {
    pub mz: Vec<f64>,
    pub intensity: Vec<f32>,
}

pub fn decode_peak_block(bytes: &[u8], params: &SpectrumParams) -> crate::Result<PeakSpectrum> {
    let point_count = params.point_count as usize;
    let expected_len = match params.format_id {
        3 => point_count * 12,
        2 => point_count * 12,
        _ => {
            return Err(crate::Error::Parse(format!(
                "Unknown peak format ID: {}",
                params.format_id
            )))
        }
    };

    if bytes.len() < expected_len {
        return Err(crate::Error::Parse("Truncated MSPeak.bin block".into()));
    }

    let mut mz = Vec::with_capacity(point_count);
    let mut intensity = Vec::with_capacity(point_count);

    if params.format_id == 3 {
        // QQQ MRM: Array of Structures (mz: double, int: float)
        for i in 0..point_count {
            let offset = i * 12;
            mz.push(LittleEndian::read_f64(&bytes[offset..offset + 8]));
            intensity.push(LittleEndian::read_f32(&bytes[offset + 8..offset + 12]));
        }
    } else if params.format_id == 2 {
        // Q-TOF: Structure of Arrays (mz: N * double, int: N * float)
        for i in 0..point_count {
            let offset = i * 8;
            let mz_raw = LittleEndian::read_f64(&bytes[offset..offset + 8]);
            mz.push(mz_raw / 100.0);
        }

        let int_start = point_count * 8;
        for i in 0..point_count {
            let offset = int_start + i * 4;
            intensity.push(LittleEndian::read_f32(&bytes[offset..offset + 4]));
        }
    }

    Ok(PeakSpectrum { mz, intensity })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(format_id: u32, point_count: u32) -> SpectrumParams {
        SpectrumParams {
            format_id,
            offset: 0,
            byte_count: 0,
            point_count,
            uncompressed_byte_count: None,
        }
    }

    #[test]
    fn decodes_qqq_mrm_array_of_structures() {
        // format_id 3: [mz: f64, intensity: f32] per point, 12 bytes/point.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&100.5f64.to_le_bytes());
        bytes.extend_from_slice(&10.0f32.to_le_bytes());
        bytes.extend_from_slice(&200.25f64.to_le_bytes());
        bytes.extend_from_slice(&20.0f32.to_le_bytes());

        let spectrum = decode_peak_block(&bytes, &params(3, 2)).unwrap();
        assert_eq!(spectrum.mz, vec![100.5, 200.25]);
        assert_eq!(spectrum.intensity, vec![10.0, 20.0]);
    }

    #[test]
    fn decodes_qtof_structure_of_arrays() {
        // format_id 2: N * (mz: f64, scaled by 100), then N * (intensity: f32).
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&10050.0f64.to_le_bytes()); // -> 100.5
        bytes.extend_from_slice(&20025.0f64.to_le_bytes()); // -> 200.25
        bytes.extend_from_slice(&10.0f32.to_le_bytes());
        bytes.extend_from_slice(&20.0f32.to_le_bytes());

        let spectrum = decode_peak_block(&bytes, &params(2, 2)).unwrap();
        assert_eq!(spectrum.mz, vec![100.5, 200.25]);
        assert_eq!(spectrum.intensity, vec![10.0, 20.0]);
    }

    #[test]
    fn unknown_format_id_is_an_error() {
        let err = decode_peak_block(&[], &params(99, 0)).unwrap_err();
        assert!(err.to_string().contains("Unknown peak format ID"));
    }

    #[test]
    fn truncated_block_is_an_error() {
        // Claims 2 points (24 bytes needed) but only provides 4.
        let bytes = vec![0u8; 4];
        let err = decode_peak_block(&bytes, &params(3, 2)).unwrap_err();
        assert!(err.to_string().contains("Truncated MSPeak.bin block"));
    }
}
