# Known Limitations

This document lists the known limitations and edge cases of the current `OpenARaw` reader that result in unparseable data or conformance invariant violations across the PRIDE corpus.

## 1. Malformed or Corrupted Folders

Some files uploaded to PRIDE are structurally malformed, missing the mandatory `AcqData` directory entirely.

**Affected Files (6 files):**
- **PXD041903**: `20190423_Alex11.d`, `20190423_Alex3.d`, `20190423_Alex7.d` (Double-nested folders missing `AcqData` in the top level).
- **PXD049393**: `__MACOSX/2022-07-07_sequence1.d`, `__MACOSX/2023-03-01_sequence1.d`, `__MACOSX/2023-07-12_sequence1.d` (macOS filesystem artifacts rather than real datasets).

## 2. Spectrum Fields Not Recoverable From `MSScan.bin`

`SpectrumRecord::polarity` and `PrecursorInfo::selected_mz` are always
`None` in the reader's output. Both were investigated against the corpus
(not just assumed absent) and found unrecoverable from the current
record layout:

- **`polarity`**: `PXD031771/526b_1.d` (and four sibling bundles in the
  same project) is a genuine mixed-polarity run - its `AcqMethod.xml`
  defines per-time-segment `<ionPolarity>` with an explicit
  Positive/Negative/Positive sequence across `StartTime` boundaries.
  Every byte, `u16`, and `u32` offset in the `ScanRecord` (0..stride) was
  checked for a value that stays constant within each polarity segment
  and differs across segments; none was found. `MSPeriodicActuals.bin`'s
  `MSActualDefs.xml` does define an `ActualID=65` "Ion Polarity" channel
  (`Unit="0=Postive,1=Negative"`), but decoding it for this bundle
  produces only 3 sparse log-on-change events with non-boolean values
  (`0.0, 0.0, 4.232`) - inconclusive, not a confirmed source, so it is
  not wired in either.
- **`selected_mz`**: distinct from `target_mz` (the isolation window
  center, which the reader already exposes via
  `PrecursorInfo::target_mz`). Every `f32`/`f64` offset in the full
  record was scanned across hundreds of MS2 scans spanning strides 216,
  220, and 284, looking for a second m/z-like value near `target_mz`;
  none was found. Either Agilent does not store a distinct
  selected/monoisotopic precursor m/z in this record (recovering one
  would require re-picking against the isolation window in `MSPeak.bin`,
  a processing step this reader does not perform), or it is identical to
  `target_mz` and therefore not a distinct field.

If either field turns out to be recoverable via a source not yet
checked (e.g. a fuller decode of `MSPeriodicActuals.bin`'s per-channel
`Value` semantics), treat this section as the place to update once
there's corpus evidence to cite - not just a plausible byte offset.

## 3. No Ion Mobility / CCS Support (Agilent 6560 IM-QTOF)

`SpectrumRecord::inv_mobility` and `SpectrumRecord::inv_mobility_per_peak`
are always `None`; `RunMetadata::mobility_array_kind` is always `None` as
well. This is a different situation from section 2 above: those two
fields were investigated against real records and found absent from the
layout. Ion mobility has never been investigated against a real record,
because **no ion-mobility acquisition exists anywhere in the validation
corpus to investigate.**

All 338 cataloged `.d` directories come from the PRIDE Archive (see
`CORPUS.md`), a proteomics-focused repository. Every `Devices.xml` mass
spectrometer `<ModelNumber>` seen across the corpus is one of `G6410A`
(QQQ), or `G6530A`/`G6530B`/`G6540A`/`G6540B`/`G6550A`/`G6550B` (Q-TOF) -
see `docs/format/01-msscan.md` and `docs/format/07-run-metadata.md`. No
`G6560` (6560 IM-QTOF) unit, or any other drift-tube-equipped instrument,
appears in any cataloged bundle.

Without a real 6560 `AcqData` bundle to read, there is nothing to
reverse-engineer against - and per this project's clean-room policy,
guessing at a plausible-looking field or file name from general IM-MS
knowledge and shipping it as a "decode" is exactly the kind of
unverified claim the policy prohibits. This is therefore left
undocumented-as-a-format-question rather than implemented:

- It is not confirmed whether a 6560 `AcqData` directory even reuses
  `MSScan.bin`'s existing record layout (e.g. an extra drift-time field
  appended to the strides already catalogued in
  `docs/format/01-msscan.md`), or whether IM-QTOF acquisitions use
  additional binary stream(s) alongside it (frame-indexed drift spectra,
  the way OpenWRaw and OpenTimsTDF each expose an instrument-specific
  mobility file distinct from their base spectrum index) - this crate
  has never seen one to check.
- It is not confirmed what such a file would be named, or what its
  header/record layout would be, without a sample to inspect.
- CCS (collision cross section) is typically a derived value (from
  drift time, charge state, and instrument calibration constants) rather
  than a directly stored field in most vendor IM formats; whether Agilent
  stores a precomputed CCS anywhere in the bundle, or only raw drift
  time requiring the reader to apply a calibration, is unknown without a
  sample.

**Next step if this is picked up again:** obtain a real 6560 IM-QTOF `.d`
bundle from a public, non-vendor-tool-verified source (a PRIDE, MetaboLights,
or similar open-data submission using this instrument) and add it to the
corpus before writing any parsing code. Until then, `inv_mobility` /
`inv_mobility_per_peak` / `mobility_array_kind` should stay `None` rather
than be backed by an unverified field guess.
