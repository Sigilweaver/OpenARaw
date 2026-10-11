//! MSProfile.bin (Profile data) parser.
//!
//! A profile block holds a flight-time axis header and one intensity per
//! point:
//!
//! | Bytes      | Field                                   |
//! |------------|-----------------------------------------|
//! | 0-7        | `x0`: flight time of the first point (f64) |
//! | 8-15       | `dx`: sampling period (f64)             |
//! | 16-        | `PointCount` intensities                |
//!
//! Two block encodings occur, told apart by the block bytes:
//!
//! - [`ProfileEncoding::Lzf`]: the whole layout above, with the intensities
//!   as int32, is one raw LZF stream starting at byte 0.
//! - [`ProfileEncoding::RunLength`]: the axis header is stored as is,
//!   followed by [`RUN_LENGTH_MARKER`] and a run-length stream of
//!   variable-width integers (see [`decode_run_length`]).
//!
//! Point `i` sits at flight time `x0 + i * dx`, converted to m/z with the
//! scan's [`TofCalibration`]. See `docs/format/04-msprofile.md`.

use crate::raw::calibration::TofCalibration;
use crate::raw::lzf;
use crate::raw::msscan::SpectrumParams;
use byteorder::{ByteOrder, LittleEndian};

/// Size of the `x0` / `dx` axis header.
pub const AXIS_HEADER_SIZE: usize = 16;

/// The 4 bytes after the axis header of a [`ProfileEncoding::RunLength`]
/// block.
pub const RUN_LENGTH_MARKER: [u8; 4] = [0x00, 0xff, 0x02, 0x90];

/// How a profile block stores its axis header and intensities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProfileEncoding {
    /// Raw LZF stream of the axis header and int32 intensities.
    Lzf,
    /// Axis header as is, then [`RUN_LENGTH_MARKER`] and a run-length
    /// stream (see [`decode_run_length`]).
    RunLength,
}

impl ProfileEncoding {
    /// The encoding of a profile block: [`ProfileEncoding::RunLength`] when
    /// the block starts with a valid axis header (positive flight time and
    /// sampling period) followed by [`RUN_LENGTH_MARKER`],
    /// [`ProfileEncoding::Lzf`] otherwise.
    pub fn detect(block: &[u8]) -> Self {
        let marker_end = AXIS_HEADER_SIZE + RUN_LENGTH_MARKER.len();
        if block.len() >= marker_end
            && block[AXIS_HEADER_SIZE..marker_end] == RUN_LENGTH_MARKER
            && ProfileAxis::from_bytes(&block[..AXIS_HEADER_SIZE]).is_ok_and(|a| a.x0 > 0.0)
        {
            ProfileEncoding::RunLength
        } else {
            ProfileEncoding::Lzf
        }
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ProfileSpectrum {
    pub mz: Vec<f64>,
    pub intensity: Vec<f32>,
}

/// The flight-time axis header of a profile block.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
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

    let (axis, intensity) = match ProfileEncoding::detect(bytes) {
        ProfileEncoding::Lzf => {
            let dec_bytes = lzf::decompress(bytes, uncompressed_len)?;
            let axis = ProfileAxis::from_bytes(&dec_bytes)?;
            let intensity = (0..point_count)
                .map(|i| {
                    let offset = AXIS_HEADER_SIZE + i * 4;
                    LittleEndian::read_i32(&dec_bytes[offset..offset + 4]) as f32
                })
                .collect();
            (axis, intensity)
        }
        ProfileEncoding::RunLength => {
            let axis = ProfileAxis::from_bytes(&bytes[..AXIS_HEADER_SIZE])?;
            let intensity = decode_run_length(&bytes[AXIS_HEADER_SIZE..], point_count)?
                .into_iter()
                .map(|v| v as f32)
                .collect();
            (axis, intensity)
        }
    };

    let mz = (0..point_count)
        .map(|i| calibration.mz(axis.x0 + i as f64 * axis.dx))
        .collect();

    Ok(ProfileSpectrum { mz, intensity })
}

/// Decode the intensities of a [`ProfileEncoding::RunLength`] block.
///
/// `stream` is the block after the axis header. Layout (little-endian):
///
/// | Bytes | Field |
/// |-------|-------|
/// | 0-3   | [`RUN_LENGTH_MARKER`] |
/// | 4-7   | int32: minus the index of the first encoded point |
/// | 8-    | integers of the current width, starting at 4 bytes |
///
/// Each integer `v` of the stream is read at the current width (1, 2, 4
/// or 8 bytes, signed):
///
/// - `v >= 0`: the intensity of the current point; move to the next point.
/// - `v < 0`: with `u = !v` (that is `-v - 1`), skip `u >> 2` points and
///   set the width to `1 << (u & 3)` bytes.
///
/// Points the stream does not set are zero. The stream ends with the
/// block. Returns `point_count` intensities; a stream that is truncated,
/// starts before point 0 or runs past `point_count` is an error, as is a
/// `point_count` whose int32 layout would exceed the LZF output cap.
pub fn decode_run_length(stream: &[u8], point_count: usize) -> crate::Result<Vec<i64>> {
    const HEADER: usize = 8;
    // `point_count` is file-controlled and sizes the output; same cap as
    // an LZF block's output.
    if point_count > lzf::MAX_DECOMPRESSED_LEN / 4 {
        return Err(crate::Error::Parse(format!(
            "run-length profile block of {point_count} points exceeds the size cap"
        )));
    }
    if stream.len() < HEADER || stream[..RUN_LENGTH_MARKER.len()] != RUN_LENGTH_MARKER {
        return Err(crate::Error::Parse(
            "run-length profile stream has no marker".into(),
        ));
    }
    let first = -i64::from(LittleEndian::read_i32(&stream[4..8]));
    let mut pos = usize::try_from(first)
        .ok()
        .filter(|&p| p <= point_count)
        .ok_or_else(|| {
            crate::Error::Parse(format!(
                "run-length profile stream starts at point {first} of {point_count}"
            ))
        })?;

    let mut out = vec![0i64; point_count];
    let mut width = 4usize;
    let mut i = HEADER;
    while i < stream.len() {
        let Some(raw) = stream.get(i..i + width) else {
            return Err(crate::Error::Parse(format!(
                "run-length profile stream truncated: {} bytes left at width {width}",
                stream.len() - i
            )));
        };
        let v = match width {
            1 => i64::from(raw[0] as i8),
            2 => i64::from(LittleEndian::read_i16(raw)),
            4 => i64::from(LittleEndian::read_i32(raw)),
            _ => LittleEndian::read_i64(raw),
        };
        i += width;
        if v >= 0 {
            let slot = out.get_mut(pos).ok_or_else(|| {
                crate::Error::Parse(format!(
                    "run-length profile stream sets point {pos} of {point_count}"
                ))
            })?;
            *slot = v;
            pos += 1;
        } else {
            let u = !v as u64;
            pos = usize::try_from(u >> 2)
                .ok()
                .and_then(|skip| pos.checked_add(skip))
                .filter(|&p| p <= point_count)
                .ok_or_else(|| {
                    crate::Error::Parse(format!(
                        "run-length profile stream skips {} points from point {pos} of {point_count}",
                        u >> 2
                    ))
                })?;
            width = 1 << (u & 3);
        }
    }
    Ok(out)
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

    /// Run-length block: axis header, marker, first point, then the
    /// stream (see `decode_run_length`).
    fn run_length_block(x0: f64, dx: f64, first: i32, stream: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&x0.to_le_bytes());
        out.extend_from_slice(&dx.to_le_bytes());
        out.extend_from_slice(&RUN_LENGTH_MARKER);
        out.extend_from_slice(&(-first).to_le_bytes());
        out.extend_from_slice(stream);
        out
    }

    /// The first 28 bytes of a real PXD044639 (acquisition software 10.1)
    /// profile block, and the bytes that follow them.
    #[test]
    fn real_run_length_block_prefix() {
        let block = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0xd2, 0xd4, 0x40, // x0 = 21320.0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe0, 0x3f, // dx = 0.5
            0x00, 0xff, 0x02, 0x90, // marker
            0x18, 0xbe, 0xff, 0xff, // first point 16872
            0xff, 0xff, 0xff, 0xff, // skip 0, width 1
            0x04, 0xdf, 0x06, 0xef, 0x01, 0x16, 0x19, // 1-byte values and skips
            0xfe, // width 2
            0x83, 0x00, 0xff, 0xff, // 131, width 1
            0x4e,
        ];
        assert_eq!(ProfileEncoding::detect(&block), ProfileEncoding::RunLength);
        assert_eq!(
            ProfileAxis::from_bytes(&block[..AXIS_HEADER_SIZE]).unwrap(),
            ProfileAxis {
                x0: 21320.0,
                dx: 0.5
            }
        );
        let v = decode_run_length(&block[AXIS_HEADER_SIZE..], 16_900).unwrap();
        // 0x04 at 16872; 0xdf = -33 skips 8; 0x06; 0xef = -17 skips 4;
        // 0x01, 0x16, 0x19; 0xfe = -2 width 2; 131; -1 width 1; 0x4e.
        let mut expected = vec![0i64; 16_900];
        expected[16_872] = 4;
        expected[16_881] = 6;
        expected[16_886] = 1;
        expected[16_887] = 22;
        expected[16_888] = 25;
        expected[16_889] = 131;
        expected[16_890] = 78;
        assert_eq!(v, expected);
    }

    #[test]
    fn run_length_switches_width() {
        let mut stream = Vec::new();
        stream.extend_from_slice(&(-2i32).to_le_bytes()); // width 2
        stream.extend_from_slice(&300i16.to_le_bytes());
        stream.extend_from_slice(&(-3i16).to_le_bytes()); // width 4
        stream.extend_from_slice(&70_000i32.to_le_bytes());
        stream.extend_from_slice(&(!((2i32 << 2) | 3)).to_le_bytes()); // skip 2, width 8
        stream.extend_from_slice(&5_000_000_000i64.to_le_bytes());
        stream.extend_from_slice(&(!(1i64 << 2)).to_le_bytes()); // skip 1, width 1
        stream.push(7);
        let block = run_length_block(1000.0, 0.5, 1, &stream);
        let v = decode_run_length(&block[AXIS_HEADER_SIZE..], 10).unwrap();
        assert_eq!(v, vec![0, 300, 70_000, 0, 0, 5_000_000_000, 0, 7, 0, 0]);
    }

    #[test]
    fn decodes_run_length_block_on_calibrated_axis() {
        let cal = calibration();
        // width 1 at point 0: 3, skip 1, 9.
        let block = run_length_block(30000.0, 0.5, 0, &[0xff, 0xff, 0xff, 0xff, 3, 0xfb, 9]);
        let spectrum = decode_profile_block(&block, &params(4, Some(16 + 16)), &cal).unwrap();
        assert_eq!(spectrum.intensity, vec![3.0, 0.0, 9.0, 0.0]);
        for (i, mz) in spectrum.mz.iter().enumerate() {
            assert_eq!(*mz, cal.mz(30000.0 + i as f64 * 0.5));
        }
    }

    #[test]
    fn run_length_errors() {
        let decode = |first: i32, stream: &[u8], n: usize| {
            let block = run_length_block(1000.0, 0.5, first, stream);
            decode_run_length(&block[AXIS_HEADER_SIZE..], n)
        };
        // Value past the last point.
        assert!(decode(2, &[0xff, 0xff, 0xff, 0xff, 1], 2).is_err());
        // Skip past the last point.
        assert!(decode(0, &[0xff, 0xff, 0xff, 0xff, 0xf3], 2).is_err());
        // First point past the end, or before point 0.
        assert!(decode(3, &[], 2).is_err());
        assert!(decode(-1, &[], 2).is_err());
        // Truncated 4-byte value.
        assert!(decode(0, &[1, 0], 2).is_err());
        // Missing marker.
        assert!(decode_run_length(&[0u8; 8], 2).is_err());
        // Point count beyond the size cap.
        assert!(decode(0, &[], usize::MAX).is_err());
        // Ending exactly at the last point is fine.
        assert_eq!(
            decode(0, &[0xff, 0xff, 0xff, 0xff, 1, 2], 2).unwrap(),
            vec![1, 2]
        );
    }

    #[test]
    fn lzf_block_is_not_detected_as_run_length() {
        let raw = uncompressed(30000.0, 0.5, &[1, 2, 3]);
        assert_eq!(
            ProfileEncoding::detect(&encode_literals(&raw)),
            ProfileEncoding::Lzf
        );
        let prefix = [
            0x07, 0x00, 0x00, 0x00, 0x00, 0x80, 0xd6, 0xd4, 0x40, 0x20, 0x06, 0x20, 0x00, 0x01,
            0xe0, 0x3f, 0x00, 0xff, 0x02, 0x90,
        ];
        // The real LZF prefix with the marker appended: its first 8 bytes
        // read as a negative flight time, so it is not an uncompressed
        // axis header.
        assert_eq!(ProfileEncoding::detect(&prefix), ProfileEncoding::Lzf);
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
