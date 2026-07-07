---
sidebar_position: 2
---

# Scan data

Each `SpectrumRecord` yielded by `iter_spectra` has its `mz`/`intensity`
arrays filled from one of two on-disk payload files, chosen by the
`SpectrumParamsType` block(s) recorded for that scan in `MSScan.bin`:

- **`MSProfile.bin`** (profile mode) - LZF-compressed `int32` ADC/intensity
  counts. The reader decompresses the block and converts it to `f64`
  m/z / `f32` intensity pairs using the scan's declared mass range
  (`MinX`/`MaxX`) as the axis. See [MSProfile.bin](../format/msprofile).
- **`MSPeak.bin`** (centroid or MRM) - already-calibrated peak lists. Q-TOF
  centroid data is stored as parallel arrays (m/z array, then intensity
  array); QQQ MRM data is one `(mz, intensity)` pair per scan. See
  [MSPeak.bin](../format/mspeak).

If a scan has both a profile and a centroid block (seen on some Q-TOF
acquisitions), the reader prefers profile data and falls back to centroid
only if the profile decode is empty.

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
