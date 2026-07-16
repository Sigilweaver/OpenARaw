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

#[cfg(test)]
mod tests {
    use super::*;

    fn params(point_count: u32, uncompressed_byte_count: Option<u32>) -> SpectrumParams {
        SpectrumParams {
            format_id: 1,
            offset: 0,
            byte_count: 0,
            point_count,
            uncompressed_byte_count,
        }
    }

    /// LZF-encode `data` as a single literal run (only valid for <= 32
    /// bytes, which covers the small fixtures used here). Mirrors the
    /// equivalent test helper in `lzf::tests` -- there's no encoder in the
    /// crate itself, this is test-only.
    fn encode_literal(data: &[u8]) -> Vec<u8> {
        assert!(data.len() <= 32);
        let mut out = vec![(data.len() - 1) as u8];
        out.extend_from_slice(data);
        out
    }

    #[test]
    fn decodes_minimal_profile_block() {
        // Uncompressed payload: 16 reserved/padding bytes, then 2 u32
        // intensity values.
        let mut uncompressed = vec![0u8; 16];
        uncompressed.extend_from_slice(&1000u32.to_le_bytes());
        uncompressed.extend_from_slice(&2000u32.to_le_bytes());

        let mut bytes = vec![0u8; 16]; // block preamble, ignored by the decoder
        bytes.extend(encode_literal(&uncompressed));

        let params = params(2, Some(uncompressed.len() as u32));
        let spectrum = decode_profile_block(&bytes, &params, 100.0, 200.0).unwrap();

        assert_eq!(spectrum.mz, vec![100.0, 200.0]);
        assert_eq!(spectrum.intensity, vec![1000.0, 2000.0]);
    }

    #[test]
    fn zero_points_returns_empty_spectrum_without_decompressing() {
        // No LZF data past the 16-byte preamble: if the zero-point
        // short-circuit didn't fire before the decompress call, this
        // would error out decompressing an empty input.
        let bytes = vec![0u8; 16];
        let spectrum = decode_profile_block(&bytes, &params(0, Some(999)), 0.0, 1.0).unwrap();
        assert!(spectrum.mz.is_empty());
        assert!(spectrum.intensity.is_empty());
    }

    #[test]
    fn block_shorter_than_preamble_is_an_error() {
        let bytes = vec![0u8; 8];
        let err = decode_profile_block(&bytes, &params(1, Some(20)), 0.0, 1.0).unwrap_err();
        assert!(err.to_string().contains("too short for preamble"));
    }

    #[test]
    fn missing_uncompressed_byte_count_is_an_error() {
        let bytes = vec![0u8; 16];
        let err = decode_profile_block(&bytes, &params(1, None), 0.0, 1.0).unwrap_err();
        assert!(err.to_string().contains("Missing UncompressedByteCount"));
    }

    #[test]
    fn decompressed_data_too_short_for_point_count_is_an_error() {
        // Claims 5 points (needs 16 + 20 = 36 uncompressed bytes) but the
        // LZF stream only decompresses to 16.
        let uncompressed = vec![0u8; 16];
        let mut bytes = vec![0u8; 16];
        bytes.extend(encode_literal(&uncompressed));

        let params = params(5, Some(uncompressed.len() as u32));
        let err = decode_profile_block(&bytes, &params, 0.0, 1.0).unwrap_err();
        assert!(err.to_string().contains("too short for point count"));
    }
}
