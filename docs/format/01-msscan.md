# MSScan.bin

Binary scan index file. Always present in `.d/AcqData/`. Encodes per-scan metadata and byte offsets into the payload files (`MSPeak.bin` / `MSProfile.bin`).

## Status: Confirmed

## File Layout

`FileSize = GlobalHeaderSize + NumOfScans * RecordStride` holds exactly for every tested file with zero remainder.

The `GlobalHeaderSize` is stored as a 32-bit little-endian integer at file offset `0x58` (88).

**Validated corpus (7 files, all with zero remainder):**

| PXD       | Instrument             | Model   | HeaderSize | Stride |
|-----------|------------------------|---------|-----------|--------|
| PXD004747 | QQQ (MRM)              | G6410A  | 296       | 196    |
| PXD028295 | QQQ (MRM)              | G6410A  | 296       | 186    |
| PXD001310 | Q-TOF (Profile+Peak)   | G6550A  | 228       | 284    |
| PXD004426 | Q-TOF (Peak only)      | G6550A  | 228       | 220    |
| PXD007734 | Q-TOF (Peak only)      | G6540B  | 228       | 220    |
| PXD011212 | Q-TOF (Peak only)      | G6550A  | 228       | 220    |
| PXD014285 | Q-TOF (Peak only)      | G6550A  | 228       | 220    |
| PXD031771 | Q-TOF (Peak only)      | -       | 228       | 216    |

Instrument models were read from `Devices.xml`. `G6540B` and `G6550A` are both Q-TOF instruments; `G6410A` is a triple-quadrupole (QQQ). `AcqSoftwareVersion` in `Contents.xml` confirms the `6200 series TOF/6500 series Q-TOF` firmware label on all Q-TOF files. The two distinct QQQ strides (196 vs 186) are confirmed from two separate `G6410A` datasets with different run configurations.

MSProfile.bin files that exist but are 0 bytes (`PXD011212`, `PXD014285`) are placeholders; their scan records have no profile SpectrumParams block.

## Global Header

Little-endian. Starts with `0x01 0x01 0x00 0x00` (value 257 as int32). The `GlobalHeaderSize` field at byte offset `0x58` is the primary structural anchor.

## Scan Record Fields (Confirmed)

All fields are little-endian. Offsets are relative to the start of each scan record (i.e., after the global header).

**Field confirmed at record offset 0 for all files:**
- `ScanID` (int32): 1-based sequential scan number.

**Field confirmed at record offset 4:**
- QQQ: MRM channel/compound ID (int32). Cycles through the defined MRM transitions per acquisition cycle.
- Q-TOF: unknown small int (often 1).

**Field confirmed at record offset 12 for all files:**
- `RetentionTime` (double, minutes): Verified monotonically increasing (zero inversions) across all scans in PXD004747 (0.0001-20.3 min), PXD028295 (2.35-41.19 min), PXD001310 (0.038-60.13 min). For QQQ MRM, all scans within an MRM cycle share the same chromatographic retention time (the MRM channels all fire at the same chromatographic point).

**Field confirmed at record offset 20:**
- `MSLevel` (int16, not int32 - the upper two bytes at offset 22 belong to a separate field and were previously misread as part of MSLevel): 1 for MS1, 2 for MS2. Observed in Q-TOF (PXD001310) where scan 8 had MSLevel=2 and had a precursor ion listed at other offsets.

**Q-TOF-specific fields (confirmed for stride=284):**
- Offset 36: TIC or related intensity metric (double).
- Offset 44: Related intensity metric (double).
- Offset 76: Collision Energy in eV (f64, 8 bytes), immediately preceding the target m/z field. Confirmed for MS2 records across strides 216/220/284 by cross-referencing against `AcqMethod.xml`'s "Ramped Collision Energy" formula (`CE = slope * (mz/100) + offset`, per precursor charge state) in PXD001310 (e.g. scan 2, precursor m/z 510.93, charge 3: stored CE=13.59 vs. formula-predicted 13.593) and against a fixed-CE Auto-MS/MS method entry (`CE=40`) in PXD031771/526b_1.d, where the field reads a constant `40.0` for the matching precursor. Not confirmed for QQQ MRM records (strides 186/196), which don't use this field - see `docs/format/06-known-limitations.md`.
- Offset 84: Precursor Target m/z (f64, 8 bytes). Confirmed for MS2 records in Q-TOF data across multiple datasets (e.g. PXD004426, PXD007734, PXD001310). Represents the center of the isolation window for Auto-MS/MS scans.
- Offset 244: `MinX` - minimum m/z of the scan window (double, Da). Verified equal to the scan's declared mass range lower bound.
- Offset 252: `MaxX` - maximum m/z of the scan window (double, Da).

## SpectrumParamsType Block (Confirmed)

Each scan record contains one or two `SpectrumParamsType` blocks. For files with both profile and centroid data (PXD001310), a block with `SpectrumFormatID=1` appears at record offset 156 and a block with `SpectrumFormatID=2` appears at record offset 220.

### SpectrumFormatID Values (Verified)

These IDs are NOT run-level properties. They are per-scan and per-block:

- **ID = 1**: Profile-mode data, pointing into `MSProfile.bin`. Only observed in PXD001310 (Q-TOF with profile mode enabled). Stores `SpectrumOffset` (int64), `ByteCount` (int32, compressed size), `PointCount` (int32, uncompressed point count), `UncompressedByteCount` (int32).
- **ID = 2**: Centroid peak data, pointing into `MSPeak.bin`. Observed in all Q-TOF files. Does NOT have an `UncompressedByteCount` field (data is not compressed).
- **ID = 3**: Single-point MRM data, pointing into `MSPeak.bin`. Observed in both QQQ files (PXD004747, PXD028295). Structurally same as ID=2 but always has `PointCount=1`.

> **Note on ID=3 encoding:** In PXD028295 (stride=186), the `SpectrumFormatID` is stored as **int16** (not int32) at the params block offset, with the `SpectrumOffset` (int64) immediately following at +2 bytes. In all other files (stride >= 220), the `SpectrumFormatID` is int32 and `SpectrumOffset` starts at +4 bytes. This is the only structural divergence found across the 7 files.

### SpectrumParams Block Offsets Within Record

| Stride | First block offset | FormatID width |
|--------|-------------------|----------------|
| 186    | 136               | int16          |
| 196    | 144               | int32          |
| 216    | 152               | int32          |
| 220    | 156               | int32          |
| 284    | 156 (profile) + 220 (centroid) | int32 |

### Block Field Layout (FormatID as int32)

| Bytes | Field             | Type   |
|-------|-------------------|--------|
| 0-3   | SpectrumFormatID  | int32  |
| 4-11  | SpectrumOffset    | int64  |
| 12-15 | ByteCount         | int32  |
| 16-19 | PointCount        | int32  |
| 20-23 | UncompressedByteCount | int32 (only present when FormatID=1) |

## Unresolved

- The `ScanID` field in PXD028295 starts at 1116 instead of 1, suggesting MSScan.bin may represent only a portion of a longer run, or scan numbering is session-level rather than file-level.
- Full field map for the bytes between `RetentionTime` (offset 12) and the first confirmed MSLevel field or mass-range fields.
- The source of the 254 SpectrumOffset inversions in PXD028295 is not yet explained. The inversions appear at MRM channel boundaries rather than being random, which may indicate that within a multi-channel MRM cycle some channels are written to MSPeak.bin out of scan-number order. This is a known consequence of interleaved multi-channel MRM acquisition rather than a format error.
