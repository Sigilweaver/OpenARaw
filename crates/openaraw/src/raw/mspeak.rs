//! MSPeak.bin (Centroid peak data) parser.

use byteorder::{ByteOrder, LittleEndian};
use crate::raw::msscan::SpectrumParams;

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
        _ => return Err(crate::Error::Parse(format!("Unknown peak format ID: {}", params.format_id))),
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
