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
                return Err(crate::Error::Parse("LZF: literal run out of bounds".to_string()));
            }
            out[oidx..oidx + ctrl].copy_from_slice(&input[iidx..iidx + ctrl]);
            iidx += ctrl;
            oidx += ctrl;
        } else {
            // Back reference
            let mut length = ctrl >> 5;
            let mut ref_idx = oidx.checked_sub(((ctrl & 0x1f) << 8) + 1)
                .ok_or_else(|| crate::Error::Parse("LZF: invalid back reference".to_string()))?;

            if length == 7 {
                if iidx >= in_len {
                    return Err(crate::Error::Parse("LZF: truncated length byte".to_string()));
                }
                length += input[iidx] as usize;
                iidx += 1;
            }

            if iidx >= in_len {
                return Err(crate::Error::Parse("LZF: truncated reference byte".to_string()));
            }
            ref_idx = ref_idx.checked_sub(input[iidx] as usize)
                .ok_or_else(|| crate::Error::Parse("LZF: invalid back reference".to_string()))?;
            iidx += 1;
            length += 2;

            if oidx + length > expected_len {
                return Err(crate::Error::Parse("LZF: match run exceeds output bounds".to_string()));
            }

            // Copy bytes one by one because ranges can overlap
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
