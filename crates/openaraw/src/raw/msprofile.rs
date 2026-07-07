//! MSProfile.bin (Profile data) parser.

use crate::raw::lzf;
use crate::raw::msscan::SpectrumParams;
use byteorder::{ByteOrder, LittleEndian};

#[derive(Debug, Clone)]
pub struct ProfileSpectrum {
    pub mz: Vec<f64>,
    pub intensity: Vec<f32>,
}

pub fn decode_profile_block(
    bytes: &[u8],
    params: &SpectrumParams,
    min_x: f64,
    max_x: f64,
) -> crate::Result<ProfileSpectrum> {
    if bytes.len() < 16 {
        return Err(crate::Error::Parse(
            "MSProfile block too short for preamble".into(),
        ));
    }

    let uncompressed_len = params.uncompressed_byte_count.ok_or_else(|| {
        crate::Error::Parse("Missing UncompressedByteCount for profile block".into())
    })? as usize;

    let point_count = params.point_count as usize;
    if point_count == 0 {
        return Ok(ProfileSpectrum {
            mz: vec![],
            intensity: vec![],
        });
    }

    // Decompress the LZF stream starting at offset 16
    let dec_bytes = lzf::decompress(&bytes[16..], uncompressed_len)?;

    // First 16 bytes (4 ints) of uncompressed data are padding/reserved
    if dec_bytes.len() < 16 + point_count * 4 {
        return Err(crate::Error::Parse(
            "Decompressed profile block too short for point count".into(),
        ));
    }

    let mut intensity = Vec::with_capacity(point_count);
    for i in 0..point_count {
        let offset = 16 + i * 4;
        let int_val = LittleEndian::read_u32(&dec_bytes[offset..offset + 4]);
        intensity.push(int_val as f32);
    }

    let mut mz = Vec::with_capacity(point_count);
    let step = if point_count > 1 {
        (max_x - min_x) / (point_count as f64 - 1.0)
    } else {
        0.0
    };

    for i in 0..point_count {
        mz.push(min_x + (i as f64) * step);
    }

    Ok(ProfileSpectrum { mz, intensity })
}
