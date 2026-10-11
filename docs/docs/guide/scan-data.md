---
sidebar_position: 2
---

# Scan data

Each `SpectrumRecord` yielded by `iter_spectra` has its `mz`/`intensity`
arrays filled from one of two on-disk payload files, chosen by the
`SpectrumParamsType` block(s) recorded for that scan in `MSScan.bin`:

- **`MSProfile.bin`** (profile mode) - blocks holding a flight-time axis
  (`x0`, sampling period `dx`) and one integer intensity per point,
  LZF-compressed (acquisition software B.05 to B.09) or run-length
  encoded (10.1).
  The reader converts the axis to m/z with the scan's calibration. See [MSProfile.bin](../format/msprofile).
- **`MSPeak.bin`** (centroid or MRM) - peak lists. Q-TOF centroid data is
  stored as parallel arrays (flight-time array, then intensity array) and
  converted to m/z with the scan's calibration; QQQ MRM data is one
  `(mz, intensity)` pair per scan with m/z stored directly. See
  [MSPeak.bin](../format/mspeak).

Each Q-TOF scan uses its own `MSMassCal.bin` record (reference-mass
corrected) or, when the bundle has none, the `DefaultMassCal.xml` entry
for its `CalibrationID`; `Reader::calibration(i)` reports which. A Q-TOF
scan without a usable calibration is yielded as an error. See [MSMassCal.bin](../format/msmasscal)
and [Known limitations](../format/known-limitations).

If a scan has both a profile and a centroid block (seen on some Q-TOF
acquisitions), the reader uses the profile block, and the centroid block
only when the profile block holds no points. A block that cannot be read
or decoded makes the scan an error (`openaraw::Error::Scan`); the reader
does not fall back from a failed profile block to the centroid block.
Failures are logged and counted (`Reader::profile_decode_failures()`,
`Reader::centroid_decode_failures()`).

## Precursor information

MS2+ spectra populate `SpectrumRecord.precursor`:

- **Q-TOF Auto-MS/MS**: `target_mz` is read directly from the scan record
  (`MSScan.bin` offset 84) - the isolation window center Agilent's
  acquisition firmware selected for that MS2 scan.
- **QQQ MRM**: there is no isolation-window target in the same sense: each
  MS2 scan corresponds to one MRM transition, identified by a channel ID.
  The reader surfaces this as `precursor_native_id` (`mrm_channel=<id>`)
  rather than a `target_mz`.

## Retention time

`retention_time_sec` is `MSScan.bin`'s per-record `RetentionTime` (stored
on disk in minutes) converted to seconds. For QQQ MRM data, all channels
within one acquisition cycle share the same retention time - the
instrument fires through its transition list once per chromatographic
point, not once per unique retention time.
