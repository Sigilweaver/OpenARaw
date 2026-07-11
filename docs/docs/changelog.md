---
sidebar_position: 98
---

# Changelog

The canonical changelog lives at
[`CHANGELOG.md`](https://github.com/Sigilweaver/OpenARaw/blob/main/CHANGELOG.md)
in the repository root. The notes below mirror the latest state.

## 0.1.0 - 2026-07-11

First release. Published on crates.io (`openaraw`) and PyPI (`openaraw`).

- Rust reader (`openaraw`) for Agilent MassHunter `.d` directories in the
  modern `AcqData` layout, covering Q-TOF (profile and centroid) and QQQ
  (MRM) acquisitions.
- Python bindings (`openaraw-py`) exposing `RawReader`/`Spectrum` via
  PyO3.
- `examples/to_mzml.rs` for mzML export and `examples/corpus_scan.rs` for
  running the full conformance suite against an entire corpus directory.
- Q-TOF Auto-MS/MS precursor m/z extraction and QQQ MRM precursor
  mapping.
- Conformance testing broadened to the full 338-file corpus (332 pass end
  to end; the other 6 are malformed source uploads, see
  [Known limitations](./format/known-limitations)).
