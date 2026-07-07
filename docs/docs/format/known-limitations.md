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
