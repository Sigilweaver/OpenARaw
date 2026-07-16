//! Raw LZF decompression.
//!
//! MSProfile.bin uses standard LZF compression without frame headers.

/// Decompress raw LZF data.
pub fn decompress(input: &[u8], expected_len: usize) -> crate::Result<Vec<u8>> {
    let mut out = vec![0u8; expected_len];
    let mut iidx = 0;
    let mut oidx = 0;
    let in_len = input.len();

    while iidx < in_len && oidx < expected_len {
        let mut ctrl = input[iidx] as usize;
        iidx += 1;

        if ctrl < (1 << 5) {
            // Literal run
            ctrl += 1;
            if iidx + ctrl > in_len || oidx + ctrl > expected_len {
                return Err(crate::Error::Parse(
                    "LZF: literal run out of bounds".to_string(),
                ));
            }
            out[oidx..oidx + ctrl].copy_from_slice(&input[iidx..iidx + ctrl]);
            iidx += ctrl;
            oidx += ctrl;
        } else {
            // Back reference
            let mut length = ctrl >> 5;
            let mut ref_idx = oidx
                .checked_sub(((ctrl & 0x1f) << 8) + 1)
                .ok_or_else(|| crate::Error::Parse("LZF: invalid back reference".to_string()))?;

            if length == 7 {
                if iidx >= in_len {
                    return Err(crate::Error::Parse(
                        "LZF: truncated length byte".to_string(),
                    ));
                }
                length += input[iidx] as usize;
                iidx += 1;
            }

            if iidx >= in_len {
                return Err(crate::Error::Parse(
                    "LZF: truncated reference byte".to_string(),
                ));
            }
            ref_idx = ref_idx
                .checked_sub(input[iidx] as usize)
                .ok_or_else(|| crate::Error::Parse("LZF: invalid back reference".to_string()))?;
            iidx += 1;
            length += 2;

            if oidx + length > expected_len {
                return Err(crate::Error::Parse(
                    "LZF: match run exceeds output bounds".to_string(),
                ));
            }

            // Copy bytes one by one because ranges can overlap; clippy's
            // range-iterator suggestion would break the oidx/ref_idx
            // co-increment this overlapping copy relies on.
            #[allow(clippy::explicit_counter_loop)]
            for _ in 0..length {
                out[oidx] = out[ref_idx];
                oidx += 1;
                ref_idx += 1;
            }
        }
    }

    if oidx != expected_len {
        return Err(crate::Error::Parse(format!(
            "LZF: decompressed length {} != expected {}",
            oidx, expected_len
        )));
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode `data` as literal-only LZF runs (chunks of <=32 bytes, each
    /// prefixed with a ctrl byte of `len - 1`). There's no LZF encoder in
    /// this crate (it's a read-only parser), so this is a hand-rolled test
    /// fixture, not production code under test.
    fn encode_literals(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for chunk in data.chunks(32) {
            out.push((chunk.len() - 1) as u8);
            out.extend_from_slice(chunk);
        }
        out
    }

    #[test]
    fn literal_only_roundtrip() {
        let original: Vec<u8> = (0..100).map(|i| (i * 7) as u8).collect();
        let encoded = encode_literals(&original);
        let decoded = decompress(&encoded, original.len()).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn empty_input_roundtrip() {
        let decoded = decompress(&[], 0).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn back_reference_decodes_repeated_run() {
        // Literal 'a' (ctrl=0 -> 1 byte), then a back-reference of distance
        // 1 and length 8 (ctrl=192 encodes length_field=6 -> final length
        // 6+2=8, dist-1=0). Copies the preceding byte forward 8 times,
        // producing 9 'a's total.
        let encoded = [0u8, b'a', 192, 0];
        let decoded = decompress(&encoded, 9).unwrap();
        assert_eq!(decoded, vec![b'a'; 9]);
    }

    #[test]
    fn literal_run_out_of_bounds_is_an_error() {
        // ctrl=5 claims a 6-byte literal run, but no data follows.
        let err = decompress(&[5], 6).unwrap_err();
        assert!(err.to_string().contains("literal run out of bounds"));
    }

    #[test]
    fn back_reference_before_any_output_is_an_error() {
        // A back-reference as the very first token has nothing to refer
        // back to (oidx is still 0).
        let err = decompress(&[192, 0], 5).unwrap_err();
        assert!(err.to_string().contains("invalid back reference"));
    }

    #[test]
    fn truncated_extended_length_byte_is_an_error() {
        // Literal 'x' establishes oidx=1 (needed so the back-reference's
        // distance check doesn't fail first). ctrl=224 has length_field
        // ctrl>>5==7, which requires one more byte for the extended
        // length, but the input ends right after the ctrl byte.
        let err = decompress(&[0, b'x', 224], 10).unwrap_err();
        assert!(err.to_string().contains("truncated length byte"));
    }

    #[test]
    fn truncated_distance_byte_is_an_error() {
        // Literal 'x' establishes oidx=1, then a trailing back-reference
        // ctrl byte with no low-distance byte after it.
        let err = decompress(&[0, b'x', 32], 10).unwrap_err();
        assert!(err.to_string().contains("truncated reference byte"));
    }

    #[test]
    fn match_run_exceeding_output_bounds_is_an_error() {
        // Same back-reference as `back_reference_decodes_repeated_run`
        // (needs 9 bytes of output) but expected_len is only 5.
        let encoded = [0u8, b'a', 192, 0];
        let err = decompress(&encoded, 5).unwrap_err();
        assert!(err.to_string().contains("match run exceeds output bounds"));
    }

    #[test]
    fn short_input_leaves_length_mismatch_error() {
        // Input is exhausted after producing 1 byte, short of expected_len.
        let err = decompress(&[0, b'a'], 5).unwrap_err();
        assert!(err
            .to_string()
            .contains("decompressed length 1 != expected 5"));
    }
}
