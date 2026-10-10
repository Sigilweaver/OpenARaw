---
sidebar_position: 7
---

# Known Limitations

This page lists the known limitations and edge cases of the current `OpenARaw` reader that result in unparseable data across the PRIDE validation corpus (332 of 338 `.d` directories pass full conformance checking end to end).

## Malformed or Corrupted Folders

Some files uploaded to PRIDE are structurally malformed, missing the mandatory `AcqData` directory entirely.

**Affected Files (6 files):**
- **PXD041903**: `20190423_Alex11.d`, `20190423_Alex3.d`, `20190423_Alex7.d` (Double-nested folders missing `AcqData` in the top level).
- **PXD049393**: `__MACOSX/2022-07-07_sequence1.d`, `__MACOSX/2023-03-01_sequence1.d`, `__MACOSX/2023-07-12_sequence1.d` (macOS filesystem artifacts rather than real datasets).

These are not format variants OpenARaw fails to handle - they are not
valid `.d` directories at all, and any reader would reject them the same
way.

## Q-TOF Calibration Files and Profile Encoding

- **Q-TOF bundles with neither `MSMassCal.bin` nor `DefaultMassCal.xml`
  yield empty spectra.** Without either file there is no calibration to
  convert flight times to m/z. The reader logs this, emits the scans with
  empty arrays, and counts each block in
  `Reader::centroid_decode_failures()` /
  `Reader::profile_decode_failures()`; flight times are never reported as
  m/z. One corpus bundle is affected: `PXD031771/655_2.d`, which also
  lacks `Devices.xml` and the other method files.
- **Profile blocks from acquisition software 10.x are not decoded.** In
  `PXD044639` (G6550B, `AcqSoftwareVersion` "6500 series Q-TOF 10.1"),
  each `MSProfile.bin` block stores the 16-byte `x0` / `dx` header
  uncompressed, followed by an encoding that is not LZF. The reader logs
  and counts each such block (`Reader::profile_decode_failures()`). These
  bundles hold profile data only, so their spectra are empty. All other
  profile bundles in the corpus (acquisition software B.05 to B.09)
  decode.
- **`PXD022473/M.d` has no `MSProfile.bin`** although its scan records
  point into one (profile data only). Every scan is logged, counted as a
  profile decode failure and emitted empty. This is a defect of the
  upload, not a format variant.
- QQQ (MRM) centroid data stores m/z directly and is unaffected.
