# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial Rust reader (`openaraw`) for Agilent MassHunter `.d` directories
  in the modern `AcqData` layout, covering Q-TOF (profile and centroid)
  and QQQ (MRM) acquisitions.
- Python bindings (`openaraw-py`) exposing `RawReader`/`Spectrum` via PyO3.
- `examples/to_mzml.rs` for mzML export and `examples/corpus_scan.rs` for
  running the full conformance suite against an entire corpus directory.
- Q-TOF Auto-MS/MS precursor m/z extraction (record offset 84 in
  `MSScan.bin`), previously undocumented.
- QQQ MRM precursor mapping: the MRM channel/compound ID is now surfaced
  as `precursor_native_id` on MS2 spectra.

### Fixed

- `MSScan.bin` record-stride detection could silently pick the wrong
  stride when more than one candidate evenly divided the payload length
  (e.g. 216 vs 196 bytes); it now validates a candidate stride against
  the next record's `ScanID`/`MSLevel` before accepting it.
- `MSLevel` was read as a 32-bit field at record offset 20; it is
  actually 16 bits, and the extra two bytes were corrupting adjacent
  data on Q-TOF records.

### Testing

- Conformance testing now runs the shared `openproteo_core` invariant
  suite (previously ad-hoc assertions) and covers the full corpus
  (332 of 338 real-world files pass end to end; the other 6 are
  malformed source uploads, not reader bugs - see
  [docs/format/05-known-limitations.md](docs/format/05-known-limitations.md)),
  up from 2 fixture files previously.
