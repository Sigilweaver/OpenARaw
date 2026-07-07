---
sidebar_position: 6
---

# MSMassCal.bin

Binary mass calibration file. Present only in Q-TOF acquisitions in the corpus. Stores per-scan dynamic recalibration coefficients that refine the default calibration stored in `DefaultMassCal.xml`.

## Status: Confirmed

## File Layout

- **Global header:** 72 bytes (derived from `(FileSize - 72) / NumOfScans = 84` with zero remainder for PXD001310, n=13301 scans).
- **Record stride:** 84 bytes.
- **Record count:** Exactly equals the number of scans in `MSScan.bin` (one calibration record per scan).

## Record Layout (84 bytes, little-endian)

| Bytes | Field                | Type    |
|-------|----------------------|---------|
| 0-3   | NumCoefficients      | int32 (always 10 in PXD001310) |
| 4-83  | Coefficients[0..9]   | double[10] (8 bytes each = 80 bytes) |

The 10 coefficients encode two calibration steps, matching the structure in `DefaultMassCal.xml`:
- **Coefficients[0..1]:** `CalibrationFormula=Traditional` (2 coefficients). For the `Traditional` formula, coeff[0] = A and coeff[1] = B in the Agilent Q-TOF calibration relation. Coeff[0] (A) varies slightly per scan as the dynamic recalibration corrects for instrumental drift.
- **Coefficients[2..9]:** `CalibrationFormula=Polynomial` (8 coefficients, last 2 are always 0.0).

## Calibration Coefficient Drift (PXD001310)

Across 13301 scans, `coeff[0]` (the Traditional A parameter) varies from `0.0003479961` to `0.0003479973`. The default XML value is `0.000347994641`. The per-scan variation is ~3.48 ppm of the coefficient magnitude - this is the dynamic mass recalibration adjustment applied during acquisition (e.g., via a co-eluting reference mass lock compound).

All other 9 coefficients remain identical to the `DefaultMassCal.xml` values across all scans in PXD001310.

## Calibration Application Note

The calibration coefficients stored in this file are **already applied** to the centroid data stored in `MSPeak.bin` by the Agilent acquisition firmware/software prior to disk write. The `mz_raw` values in `MSPeak.bin` are fully calibrated (stored as `mz * 100` retaining full double precision). Therefore, these coefficients are recorded for traceability and metadata purposes, but are not required to be manually applied to the raw flight times or raw m/z values to read the final centroid spectra.

## Corpus Evidence

- PXD001310 (G6550A Q-TOF): header=72, stride=84, records=13301. Record 1 coefficients match `DefaultCalibration DefaultCalibrationID="1"` in `DefaultMassCal.xml` to full double precision. Record 2 matches `DefaultCalibrationID="1"` for coeff[1..9] and differs only in coeff[0] by the drift value documented above.
- PXD004747 (QQQ): No `MSMassCal.bin` present - consistent with QQQ not using TOF-specific mass calibration.

## Unresolved

- The exact meaning of the 72-byte header is not yet mapped.
