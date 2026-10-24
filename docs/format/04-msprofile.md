# MSProfile.bin

Binary payload file containing profile-mode (full-scan) mass spectra. Only present when the instrument was configured to record profile data. In all corpus files tested, if `MSProfile.bin` exists with non-zero size, the corresponding `MSScan.bin` scan records have a SpectrumFormatID=1 block pointing into it.

## Status: Confirmed

Two block encodings occur. Every profile block written by acquisition software B.05 to B.09 in the corpus is an LZF block; every block written by version 10.1 is a run-length block (see Evidence). The reader tells them apart per block and converts the axis with the scan's mass calibration (see `05-msmasscal.md`).

## File Layout

- **Global header:** 68 bytes. The first 4 bytes are an int32 magic number/version flag: `258` (`0x02 0x01 0x00 0x00`). The remaining 64 bytes are all zeros padding.
- **Data payload:** Contiguous scan blocks, each located by `SpectrumOffset` / `ByteCount` in the scan's `SpectrumFormatID=1` block in `MSScan.bin`. `UncompressedByteCount` is `16 + PointCount * 4` in both encodings.

## Decoded Layout

| Field | Type |
|-------|------|
| `x0`: flight time of point 0 | f64 |
| `dx`: sampling period | f64 |
| `PointCount` intensities | integer counts |

- Point `i` is at flight time `x0 + i * dx`: a uniform time axis, not a uniform m/z axis.
- Flight times are converted to m/z exactly like Q-TOF centroid x-values (`03-mspeak.md`), with the scan's calibration from `05-msmasscal.md`.

## LZF Blocks

- The block is a **raw LZF stream** (no frame header) starting at **byte 0**. There is no uncompressed preamble.
- It decompresses to exactly `UncompressedByteCount` bytes, little-endian:

| Bytes | Field | Type |
|-------|-------|------|
| 0-7   | `x0` | f64 |
| 8-15  | `dx` | f64 |
| 16-   | `PointCount` intensities | int32 |

## Run-Length Blocks

Little-endian throughout:

| Bytes | Field | Type |
|-------|-------|------|
| 0-7   | `x0` | f64 |
| 8-15  | `dx` | f64 |
| 16-19 | marker `00 ff 02 90` | bytes |
| 20-23 | minus the index of the first encoded point | int32 |
| 24-   | stream | signed integers of varying width |

The stream is read as signed integers of the current width, which starts at 4 bytes. Starting at the point given by bytes 20-23, for each integer `v`:

- `v >= 0`: `v` is the intensity of the current point; move to the next point.
- `v < 0`: let `u = -v - 1` (the bitwise complement of `v`). Skip `u >> 2` points and set the width to `1 << (u & 3)` bytes (`u & 3` = 0, 1, 2, 3 for 1, 2, 4, 8 bytes).

Points the stream does not reach are zero, including those before the first encoded point and after the last. The stream ends with the block; the last point it reaches is at most `PointCount`.

A run of zeros and a change of width share one negative integer. The first integer of the stream sets the width of the first values; in the corpus it is `-1` (1-byte values) in 189,405 blocks and `-2` (2-byte values) in 3,098.

The reader detects this encoding by the marker at bytes 16-19 following a valid axis header (`x0 > 0`, `dx > 0`, both finite); any other block is decoded as LZF.

## Evidence

**LZF.** The first 16 bytes of a PXD001310 profile block are:

```
07 00 00 00  00 80 d6 d4  40 20 06 20  00 01 e0 3f
```

Read as LZF, these bytes are a literal run of 8 bytes (`0x07`), two back-references (`0x20 0x06`, `0x20 0x00`) and a 2-byte literal (`0x01`). They decompress to 16 bytes: `x0 = 21338.0` and `dx = 0.5`. A unit test in `crates/openaraw/src/raw/msprofile.rs` decodes them.

Corpus check (`qtof_calibration_check` example): every profile block in the 26 LZF profile bundles (PXD001310, PXD027071, PXD038602, PXD038606, PXD049393, PXD057471; acquisition software B.05 to B.09) decodes. Where the bundle's summary fields describe profile data, the sum of the int32 intensities equals the stored `TIC`, the most intense point equals `BasePeakValue`, and that point's calibrated m/z equals `BasePeakMZ` (worst-bundle p95 0.00045 ppm).

**Run-length.** The first block of `PXD044639/A1FU.d` starts:

```
00 00 00 00 00 d2 d4 40  00 00 00 00 00 00 e0 3f   x0 = 21320.0, dx = 0.5
00 ff 02 90                                        marker
18 be ff ff                                        -16872: first point 16872
ff ff ff ff                                        -1: skip 0, width 1
04 df 06 ef 01 16 19 fe 83 00 ff ff 4e ...
```

From point 16872: `04` is 4; `df` (-33, `u = 32`) skips 8 points; `06` is 6; `ef` (-17) skips 4; `01 16 19` are 1, 22, 25; `fe` (-2, `u = 1`) switches to 2-byte values without a skip; `83 00` is 131; `ff ff` (-1) switches back to 1-byte values; `4e` is 78. A unit test in `crates/openaraw/src/raw/msprofile.rs` decodes this prefix.

Corpus check (`tests/profile_run_length.rs`): all 192,503 blocks in the 13 PXD044639 bundles (G6550B, `AcqSoftwareVersion` "6200 series TOF/6500 series Q-TOF 10.1 (48.0)") decode. For every scan the decoded intensities sum exactly to the stored `TIC`, their maximum equals `BasePeakValue` exactly, and a point holding that maximum, converted with the `DefaultMassCal.xml` calibration, is within 0.0002 ppm of `BasePeakMZ`. 1-, 2- and 4-byte values occur; 8-byte values do not. Bytes 16-19 are `00 ff 02 90` in every block.

## Unresolved

- The meaning of the run-length marker bytes (`00 ff 02 90`); they are constant across the corpus and are only used to detect the encoding.
- 8-byte values (`u & 3 = 3`) follow from the 2-bit width code but are not observed in the corpus.
