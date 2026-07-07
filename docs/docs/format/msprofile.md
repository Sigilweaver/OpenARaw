---
sidebar_position: 5
---

# MSProfile.bin

Binary payload file containing profile-mode (full-scan) mass spectra. Only present when the instrument was configured to record profile data. In all corpus files tested, if `MSProfile.bin` exists with non-zero size, the corresponding `MSScan.bin` scan records have a SpectrumFormatID=1 block pointing into it.

## Status: Confirmed

## File Layout

- **Global header:** 68 bytes. The first 4 bytes are an int32 magic number/version flag: `258` (`0x02 0x01 0x00 0x00`). The remaining 64 bytes are all zeros padding.
- **Data payload:** Contiguous compressed scan blocks.

## Profile Block Encoding

- `PointCount`: number of intensity values in the uncompressed block. Read from `MSScan.bin` scan record.
- **Compression:** Profile blocks are compressed using the standard **LZF algorithm**.
- The uncompressed data is an array of **32-bit integers (int32)** representing raw intensity counts (e.g., ADC hits: 0, 1, 2, 3...). It is NOT an array of floats.
- `UncompressedByteCount`: The exact size of the uncompressed data in bytes. This will typically be `PointCount * 4 + 16` (to account for the preamble and padding).

### Block Byte Structure

1. **Preamble (16 bytes):**
   - The block starts with a 16-byte fixed preamble.
   - Example (PXD001310): `07 00 00 00  00 80 d6 d4  40 20 06 20  00 01 e0 3f`
   - These 16 bytes are NOT part of the LZF compressed stream.
2. **LZF Compressed Payload:**
   - Starts exactly at byte offset 16 within the block.
   - It is a **raw LZF stream** (no frame headers like `ZV`, which is why standard command-line tools fail to decompress it out-of-the-box).
   - Decompressing this raw stream using the standard LZF algorithm yields exactly `UncompressedByteCount` bytes.
3. **Uncompressed Structure:**
   - The uncompressed stream is simply an array of 32-bit integers.
   - The first 16 bytes (4 integers) of the decompressed data are typically all zeros padding.
   - The remaining integers correspond sequentially to the intensities of the profile points.
   - The m/z axis is not stored; it is a regular grid inferred from the scan's `MinX` and `MaxX` parameters divided by `PointCount`.
