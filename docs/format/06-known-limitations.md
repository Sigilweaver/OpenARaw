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
