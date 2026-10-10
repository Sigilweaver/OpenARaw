# MSProfile.bin

Binary payload file containing profile-mode (full-scan) mass spectra. Only present when the instrument was configured to record profile data. In all corpus files tested, if `MSProfile.bin` exists with non-zero size, the corresponding `MSScan.bin` scan records have a SpectrumFormatID=1 block pointing into it.

## Status: Confirmed for acquisition software B.05-B.09

Every profile block written by acquisition software B.05 to B.09 in the corpus decodes with the layout below (see Evidence). Blocks written by version 10.1 use a different encoding (see Unresolved). The reader converts the axis with the scan's mass calibration (see `05-msmasscal.md`).

## File Layout

- **Global header:** 68 bytes. The first 4 bytes are an int32 magic number/version flag: `258` (`0x02 0x01 0x00 0x00`). The remaining 64 bytes are all zeros padding.
- **Data payload:** Contiguous compressed scan blocks, each located by `SpectrumOffset` / `ByteCount` in the scan's `SpectrumFormatID=1` block in `MSScan.bin`.

## Profile Block Encoding

- Each block is a **raw LZF stream** (no frame header) starting at **byte 0** of the block. There is no uncompressed preamble.
- It decompresses to exactly `UncompressedByteCount` bytes, which is `16 + PointCount * 4`.

### Decompressed layout (little-endian)

| Bytes | Field | Type |
|-------|-------|------|
| 0-7   | `x0`: flight time of point 0 | f64 |
| 8-15  | `dx`: sampling period | f64 |
| 16-   | `PointCount` intensities | int32 |

- Point `i` is at flight time `x0 + i * dx`: a uniform time axis, not a uniform m/z axis.
- Flight times are converted to m/z exactly like Q-TOF centroid x-values (`03-mspeak.md`), with the scan's calibration from `05-msmasscal.md`.

## Evidence

The first 16 bytes of a PXD001310 profile block are:

```
07 00 00 00  00 80 d6 d4  40 20 06 20  00 01 e0 3f
```

Read as LZF, these bytes are a literal run of 8 bytes (`0x07`), two back-references (`0x20 0x06`, `0x20 0x00`) and a 2-byte literal (`0x01`). They decompress to 16 bytes: `x0 = 21338.0` and `dx = 0.5`. A unit test in `crates/openaraw/src/raw/msprofile.rs` decodes them.

Corpus check (`qtof_calibration_check` example): every profile block in the 26 decodable profile bundles (PXD001310, PXD027071, PXD038602, PXD038606, PXD049393, PXD057471; acquisition software B.05 to B.09) decodes. Where the bundle's summary fields describe profile data, the sum of the int32 intensities equals the stored `TIC`, the most intense point equals `BasePeakValue`, and that point's calibrated m/z equals `BasePeakMZ` (worst-bundle p95 0.00045 ppm).

## Unresolved

- Acquisition software 10.1 (`PXD044639`, G6550B): the block starts with the 16-byte `x0` / `dx` header stored uncompressed (first block: `x0 = 21320.0`, `dx = 0.5`), and `UncompressedByteCount` is still `16 + PointCount * 4`, but the bytes after the header are not an LZF stream. The encoding is not yet determined; the reader counts these blocks as decode failures (the bundles have no centroid data, so the spectra are empty).
