---
sidebar_position: 6
---

# MSMassCal.bin and DefaultMassCal.xml

Q-TOF mass calibration. Q-TOF flight times (`MSPeak.bin` FormatID=2 x-values, the `MSProfile.bin` axis) are converted to m/z with a two-step calibration. `MSMassCal.bin` holds one calibration per scan; `DefaultMassCal.xml` holds the defaults and the `Polynomial` step's `ValueUseFlags`, which the binary records do not store. QQQ bundles have neither file.

## Status: Confirmed

The formula below reproduces the `BasePeakMZ` stored in every Q-TOF `MSScan.bin` record to well under 0.001 ppm across the corpus (see Corpus Evidence).

## Calibration Formula

```
m/z = (A * (t - T0))^2 - sum_k c_k * clamp(t, t_min, t_max)^k
```

- `t` is the flight time as stored (centroid x-value, or `x0 + i * dx` on the profile axis).
- **`Traditional` step:** `A` and `T0`.
- **`Polynomial` step:** eight values `[t_min, t_max, c_0, ..., c_5]`. The polynomial is evaluated at `t` clamped to `[t_min, t_max]`, and its result is subtracted from the `Traditional` m/z.
- **Powers:** the set bits of the `Polynomial` step's `ValueUseFlags`, lowest bit first. Coefficient slot `j` belongs to the `j`-th set bit; slots beyond the number of set bits are zero. Example: `ValueUseFlags=232` (bits 3, 5, 6, 7) means `c_0*t^3 + c_1*t^5 + c_2*t^6 + c_3*t^7`. The 37 distinct flag values in the corpus select between 0 and 6 powers in the range 0-11.

## MSMassCal.bin Layout

- **Global header:** 72 bytes. In all 188 corpus files: int32 `260` (`0x04 0x01 0x00 0x00`) at offset 0, int32 `1` at offset 68, zeros elsewhere. Not otherwise interpreted.
- **Record stride:** 84 bytes. `(FileSize - 72)` is an exact multiple of 84.
- **Record count:** equals the number of scans in `MSScan.bin` in all 188 files; record `i` belongs to scan record `i`.

### Record Layout (84 bytes, little-endian)

| Bytes | Field            | Type |
|-------|------------------|------|
| 0-3   | NumCoefficients  | int32 (10 in every corpus record) |
| 4-83  | Coefficients     | double[10]: `A`, `T0`, then the 8 `Polynomial` values |

## DefaultMassCal.xml Layout

UTF-8 with a byte-order mark. All 222 corpus files have the same structure: two `DefaultCalibration` entries (`DefaultCalibrationID` 1 and 2), each with a `Traditional` step (`NumberOfCoefficients=2`, `ValueUseFlags=0`) and a `Polynomial` step (`NumberOfCoefficients=8`). `CalibrationTechnique` is `ExternalReference` throughout. Values are decimal text with 15 significant digits, so they equal the binary doubles to about 1e-15 relative.

`MSScan.bin` `CalibrationID` (record offset 52) names the `DefaultCalibrationID` a scan uses.

## Per-Scan Records and Reference-Mass Correction

In every corpus record, the 8 `Polynomial` values equal those of the `DefaultMassCal.xml` entry named by the scan's `CalibrationID`. `A`, and in some bundles `T0`, differ from that entry: the instrument adjusts them per scan from the infused reference-mass ions. Scans the instrument did not adjust keep the default `A` and `T0`.

The stored `BasePeakMZ` is computed with the `DefaultMassCal.xml` calibration, before this adjustment. Decoding with the per-scan record therefore differs from `BasePeakMZ` by the adjustment, and puts the reference ions at their theoretical m/z where the instrument applied it (see Corpus Evidence).

## Reader Behaviour

`crates/openaraw/src/raw/calibration.rs`, `defaultmasscal.rs` and `Reader::open`:

- Each scan uses its own `MSMassCal.bin` record. The `ValueUseFlags` come from the `DefaultMassCal.xml` entry whose `Polynomial` values equal the record's to within 1e-12 relative, preferring the entry named by the scan's `CalibrationID`. A record whose `Polynomial` coefficient slots are all zero needs no flags.
- When `MSMassCal.bin` is absent, each scan uses the `DefaultMassCal.xml` entry for its `CalibrationID` (logged at info level). This is normal: about a sixth of the corpus Q-TOF bundles have no `MSMassCal.bin`.
- When `MSMassCal.bin` is unreadable, its record count differs from the scan count, or a record is unusable, the affected scans use `DefaultMassCal.xml` instead, with a warning.
- A scan with no usable calibration from either source gets none; its payloads are logged and counted as decode failures, never reported as uncalibrated flight times.
- `Reader::calibration(i)` returns the calibration and its `CalibrationSource`.

## Corpus Evidence

Measured with `cargo run --release --example qtof_calibration_check -- <bundle.d>` on all 224 Q-TOF bundles of the research corpus (24 PRIDE accessions; models G6530A/B, G6540A/B, G6550A/B; acquisition software B.05 to 10.1):

- **Formula:** in the 209 bundles whose spectra decode (5.4 million scans), decoding the base peak with the `DefaultMassCal.xml` calibration reproduces the stored `BasePeakMZ` with a worst-bundle median of 0.00012 ppm and a worst-bundle p95 of 0.00049 ppm. 174 of these bundles use `MSMassCal.bin` records, 35 only `DefaultMassCal.xml`.
- **What `BasePeakMZ` describes:** in 185 bundles the stored `TIC` equals the summed centroid intensity and `BasePeakMZ` is the m/z of the most intense centroid; in 24 profile bundles (acquisition software B.06 to B.09) `TIC` equals the summed profile intensity, `BasePeakValue` the most intense profile point, and `BasePeakMZ` that point's m/z. PXD001310 (B.05.01) has profile data but centroid-based summary fields.
- **Without the `Polynomial` step** (`Traditional` only) the per-bundle median error is 0.56 to 33 ppm. Evaluating the polynomial without clamping to `[t_min, t_max]` leaves about 1 ppm on scans outside the range (PXD004426).
- **Reference-mass correction:** with the per-scan records, the lock mass sits within 0.1 ppm (median) of its theoretical m/z in 58 of the 60 bundles where HP-0921 (`922.009798`) is found and 53 of the 68 where HP-1221 (`1221.990637`) is found; PXD001310 HP-1221: 0.004 ppm, against about -10 ppm with the default calibration. In the remaining bundles most records keep the default `A`, i.e. the instrument did not adjust those scans.
- **Not decodable:** `PXD031771/655_2.d` has neither calibration file; the 13 `PXD044639` bundles and `PXD022473/M.d` hold profile data the reader cannot decode (see `06-known-limitations.md`).
- QQQ bundles (PXD004747, PXD028295) have neither file.

## Unresolved

- The meaning of the non-zero header words (offsets 0 and 68).
