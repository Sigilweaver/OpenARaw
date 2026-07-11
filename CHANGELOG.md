# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- `MSScan.bin` stride detection could slice out of bounds and panic on a
  malformed file whose payload was 1-3 bytes long; the stride probe now
  requires at least one full 4-byte `ScanID` before reading.

### Security

- Upgraded `pyo3` from 0.28.3 to 0.29, clearing RUSTSEC-2026-0176 and
  RUSTSEC-2026-0177. A `cargo audit` CI workflow now guards the lockfile.

### Testing

- The corpus conformance tests now skip cleanly (instead of failing the
  build) when the out-of-tree corpus is absent, e.g. on CI runners.

## [0.1.0] - 2026-07-11

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

- Conformance testing now runs the shared `openmassspec_core` invariant
  suite (previously ad-hoc assertions) and covers the full corpus
  (332 of 338 real-world files pass end to end; the other 6 are
  malformed source uploads, not reader bugs - see
  [docs/format/06-known-limitations.md](docs/format/06-known-limitations.md)),
  up from 2 fixture files previously.
