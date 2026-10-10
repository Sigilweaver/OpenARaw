//! MSProfile.bin (Profile data) parser.
//!
//! Each block is a raw LZF stream starting at byte 0 of the block. It
//! decompresses to `UncompressedByteCount` bytes laid out as:
//!
//! | Bytes      | Field                                   |
//! |------------|-----------------------------------------|
//! | 0-7        | `x0`: flight time of the first point (f64) |
//! | 8-15       | `dx`: sampling period (f64)             |
//! | 16-        | `PointCount` intensities (int32)        |
//!
//! Point `i` sits at flight time `x0 + i * dx`, converted to m/z with the
//! scan's [`TofCalibration`]. See `docs/format/04-msprofile.md`.

use crate::raw::calibration::TofCalibration;
use crate::raw::lzf;
use crate::raw::msscan::SpectrumParams;
use byteorder::{ByteOrder, LittleEndian};

/// Size of the decompressed `x0` / `dx` header.
pub const AXIS_HEADER_SIZE: usize = 16;

#[derive(Debug, Clone)]
pub struct ProfileSpectrum {
    pub mz: Vec<f64>,
    pub intensity: Vec<f32>,
}

/// The decompressed flight-time axis header of a profile block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProfileAxis {
    /// Flight time of point 0.
    pub x0: f64,
    /// Sampling period between consecutive points.
    pub dx: f64,
}

impl ProfileAxis {
    pub fn from_bytes(header: &[u8]) -> crate::Result<Self> {
        if header.len() < AXIS_HEADER_SIZE {
            return Err(crate::Error::Parse(format!(
                "profile axis header needs {AXIS_HEADER_SIZE} bytes, got {}",
                header.len()
            )));
        }
        let x0 = LittleEndian::read_f64(&header[0..8]);
        let dx = LittleEndian::read_f64(&header[8..16]);
        if !x0.is_finite() || !dx.is_finite() || dx <= 0.0 {
            return Err(crate::Error::Parse(format!(
                "invalid profile axis: x0={x0}, dx={dx}"
            )));
        }
        Ok(ProfileAxis { x0, dx })
    }
}

pub fn decode_profile_block(
    bytes: &[u8],
    params: &SpectrumParams,
    calibration: &TofCalibration,
) -> crate::Result<ProfileSpectrum> {
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

    let needed = point_count
        .checked_mul(4)
        .and_then(|n| n.checked_add(AXIS_HEADER_SIZE))
        .ok_or_else(|| crate::Error::Parse("profile point count overflows".into()))?;
    if uncompressed_len < needed {
        return Err(crate::Error::Parse(format!(
            "UncompressedByteCount {uncompressed_len} too short for {point_count} points \
             (needs {needed})"
        )));
    }

    let dec_bytes = lzf::decompress(bytes, uncompressed_len)?;
    let axis = ProfileAxis::from_bytes(&dec_bytes)?;

    let mut intensity = Vec::with_capacity(point_count);
    let mut mz = Vec::with_capacity(point_count);
    for i in 0..point_count {
        let offset = AXIS_HEADER_SIZE + i * 4;
        intensity.push(LittleEndian::read_i32(&dec_bytes[offset..offset + 4]) as f32);
        mz.push(calibration.mz(axis.x0 + i as f64 * axis.dx));
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

    fn calibration() -> TofCalibration {
        TofCalibration::traditional_only(3.47994641e-4, 12.5)
    }

    /// LZF-encode `data` as literal runs. There's no encoder in the crate
    /// itself, this is test-only.
    fn encode_literals(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for chunk in data.chunks(32) {
            out.push((chunk.len() - 1) as u8);
            out.extend_from_slice(chunk);
        }
        out
    }

    fn uncompressed(x0: f64, dx: f64, intensities: &[i32]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&x0.to_le_bytes());
        out.extend_from_slice(&dx.to_le_bytes());
        for v in intensities {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    /// The first 16 bytes of a real PXD001310 profile block (see the
    /// Evidence section of docs/format/04-msprofile.md) are an LZF stream
    /// that decompresses to the axis header: x0 = 21338.0, dx = 0.5.
    #[test]
    fn real_block_prefix_decodes_to_axis_header() {
        let prefix = [
            0x07, 0x00, 0x00, 0x00, 0x00, 0x80, 0xd6, 0xd4, 0x40, 0x20, 0x06, 0x20, 0x00, 0x01,
            0xe0, 0x3f,
        ];
        let header = lzf::decompress(&prefix, AXIS_HEADER_SIZE).unwrap();
        let axis = ProfileAxis::from_bytes(&header).unwrap();
        assert_eq!(
            axis,
            ProfileAxis {
                x0: 21338.0,
                dx: 0.5
            }
        );
    }

    #[test]
    fn decodes_profile_block_on_calibrated_axis() {
        let cal = calibration();
        let raw = uncompressed(30000.0, 0.5, &[1000, 0, 2000]);
        let bytes = encode_literals(&raw);

        let spectrum =
            decode_profile_block(&bytes, &params(3, Some(raw.len() as u32)), &cal).unwrap();

        assert_eq!(spectrum.intensity, vec![1000.0, 0.0, 2000.0]);
        assert_eq!(spectrum.mz.len(), 3);
        for (i, mz) in spectrum.mz.iter().enumerate() {
            let expected = cal.mz(30000.0 + i as f64 * 0.5);
            assert!((mz - expected).abs() < 1e-12);
        }
        assert!(spectrum.mz.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn zero_points_returns_empty_spectrum_without_decompressing() {
        let spectrum = decode_profile_block(&[], &params(0, Some(999)), &calibration()).unwrap();
        assert!(spectrum.mz.is_empty());
        assert!(spectrum.intensity.is_empty());
    }

    #[test]
    fn missing_uncompressed_byte_count_is_an_error() {
        let err = decode_profile_block(&[0u8; 16], &params(1, None), &calibration()).unwrap_err();
        assert!(err.to_string().contains("Missing UncompressedByteCount"));
    }

    #[test]
    fn uncompressed_byte_count_too_short_for_point_count_is_an_error() {
        let raw = uncompressed(30000.0, 0.5, &[1]);
        let bytes = encode_literals(&raw);
        let err = decode_profile_block(&bytes, &params(5, Some(raw.len() as u32)), &calibration())
            .unwrap_err();
        assert!(err.to_string().contains("too short for 5 points"));
    }

    #[test]
    fn invalid_sampling_period_is_an_error() {
        let raw = uncompressed(30000.0, 0.0, &[1]);
        let bytes = encode_literals(&raw);
        let err = decode_profile_block(&bytes, &params(1, Some(raw.len() as u32)), &calibration())
            .unwrap_err();
        assert!(err.to_string().contains("invalid profile axis"));
    }

    #[test]
    fn truncated_lzf_stream_is_an_error() {
        let raw = uncompressed(30000.0, 0.5, &[1, 2]);
        let mut bytes = encode_literals(&raw);
        bytes.truncate(10);
        assert!(
            decode_profile_block(&bytes, &params(2, Some(raw.len() as u32)), &calibration())
                .is_err()
        );
    }
}
