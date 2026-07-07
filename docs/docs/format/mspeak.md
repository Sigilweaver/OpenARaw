---
sidebar_position: 4
---

# MSPeak.bin

Binary payload file containing centroid (peak) mass spectra. Present in both QQQ and Q-TOF acquisitions. Each scan's data block is located via `SpectrumOffset` and `ByteCount` from the corresponding `MSScan.bin` scan record.

## Status: Confirmed

## File Layout

- **Global header:** 68 bytes. The first 4 bytes are an int32 magic number/version flag: `259` (`0x03 0x01 0x00 0x00`). The remaining 64 bytes are all zeros padding.
- **Data payload:** Contiguous scan blocks, each starting at the `SpectrumOffset` stored in `MSScan.bin`.

## Peak Block Encoding

The encoding inside each block differs between QQQ and Q-TOF.

### QQQ MRM (FormatID=3): Array of Structures (AoS)

- 12 bytes per point: `double mz (8 bytes) + float intensity (4 bytes)`.
- In MRM mode, `PointCount` is always 1 (single MRM transition per scan).
- `mz` is in Da (real mass units, no scaling factor). Verified against `AcqMethod.xml` transition list.
- Example (PXD004747 scan 1): `mz=875.4300, intensity=41.28`; (scan 2) `mz=673.3700, intensity=41.26`.

### Q-TOF Centroid (FormatID=2): Structure of Arrays (SoA)

- A block of N points is NOT stored as N interleaved (mz, intensity) tuples.
- Layout: `N * double mz_raw (8 bytes each)`, immediately followed by `N * float intensity (4 bytes each)`.
- Total block size: `N * 8 + N * 4 = N * 12`, matching `ByteCount`.
- **mz encoding:** `mz_raw` values are stored as 100x the real, fully-calibrated m/z in Da (retaining full double-precision floating point accuracy). Divide by 100.0 to get the final Da value.
  - *Note:* The calibration parameters in `MSMassCal.bin` are already applied to these values upstream by the instrument's acquisition software before they are written to disk. No further calibration math is required.
  - Verified across PXD001310 and PXD004426 by confirming that `mz_raw / 100.0` falls within the scan's `MinX`/`MaxX` bounds.
- MS2 scan centroid ions may fall outside the MinX/MaxX of the precursor scan (expected; those bounds describe the Q1/profile scan range, not the fragment window).
- Confirmed SoA on two independent Q-TOF files:
  - PXD001310 scan 1: N=1258, block=15096 bytes (10064 + 5032). `mz_raw` values strictly follow double-precision fractionals (e.g., `29888.4237296802` -> `298.88423730` Da).
  - PXD004426 scan 1: N=15, block=180 bytes (120 + 60). `mz_raw` range: 50767-101587 -> Da: 507.7-1015.9.
- Intensities are positive and non-zero for all non-baseline peaks (verified).

## Unresolved

- Whether Q-TOF files from instruments other than G6550A/G6540B use the same 100x mz scale.
