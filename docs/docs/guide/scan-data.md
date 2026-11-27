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
  acquisition firmware selected for that MS2 scan. The collision energy
  comes from offset 76 as one `collision_energies` entry whose unit is
  `None`: the file does not state it (see [MSScan.bin](../format/msscan)).
- **QQQ MRM**: there is no isolation-window target in the same sense: each
  MS2 scan corresponds to one MRM transition, identified by a channel ID.
  The reader surfaces the channel ID as `extra["openaraw.mrm_channel_id"]`.
  `target_mz`, `precursor_native_id` and `collision_energies` stay unset:
  the channel names no spectrum, and the record holds no collision energy.

## Scan mode

`scan_mode` is `Profile` when the scan's profile block was used and
`Centroid` when its centroid or MRM block was used. A scan record that
names neither block yields empty arrays and `scan_mode = None`; the
mzML writer rejects it unless the check `unknown_scan_mode` is allowed.

## Polarity

`polarity` comes from the scan record's `IonPolarity` field (`0`
positive, `1` negative), so runs that switch polarity by time segment
report it per spectrum. Any other value leaves `polarity` as `None`, with
the raw value in `extra["openaraw.ion_polarity"]`. See
[MSScan.bin](../format/msscan).

## Retention time

`retention_time_sec` is `MSScan.bin`'s per-record `RetentionTime` (stored
on disk in minutes) converted to seconds. Every spectrum has one. For QQQ MRM data, all channels
within one acquisition cycle share the same retention time - the
instrument fires through its transition list once per chromatographic
point, not once per unique retention time.
