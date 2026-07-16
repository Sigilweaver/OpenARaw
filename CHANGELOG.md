# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Testing

- Added synthetic byte-slice unit tests for the block decoders (`lzf`,
  `msscan`, `mspeak`, `msprofile`), which previously had no coverage on
  CI (the only tests were corpus-gated and skip when the corpus is
  absent). Covers LZF literal/back-reference decoding and its malformed-
  input error paths, MSScan.bin stride detection across all five
  candidate strides plus a regression test for the short-payload bounds
  guard (0.1.1), and minimal MSPeak/MSProfile record decoding. `MSScan`
  gained a `from_bytes` associated function (`from_path` now delegates
  to it) so the parser can be exercised without touching the filesystem.
  Fixes #2. (@Nabejo)

## [0.1.2] - 2026-07-15

### Fixed

- Bumped `openmassspec-core` to 1.2.0 and added the `SpectrumRecord.faims_cv`
  field it requires, fixing a build break: 1.2.0 added that field as
  required, and `Reader::iter_spectra` constructed the struct literal
  without it. Always `None` - Agilent instruments have no FAIMS interface.
- `run_metadata()`'s `instrument` field is now resolved from the mass
  spectrometer's `<ModelNumber>` in `AcqData/Devices.xml` (mapped to a
  specific PSI-MS CV term where one exists, e.g. `MS:1002784` "6550A
  iFunnel Q-TOF LC/MS", falling back to the generic `MS:1000490`
  "Agilent instrument model" otherwise), instead of guessing "Q-TOF" vs
  "QQQ" from whether `MSScan.bin`'s record stride happened to be `>=
  220`. The stride-based guess remains as a fallback for the rare bundle
  missing `Devices.xml`.
- `start_timestamp` is now parsed from `<AcquiredTime>` in
  `AcqData/Contents.xml` instead of being hardcoded to `None`.
- `PrecursorInfo::collision_energy` is now parsed from `MSScan.bin`
  record offset 76 (f64, eV) for MS2 Auto-MS/MS records, instead of
  being hardcoded to `None`. Confirmed by cross-referencing against
  `AcqMethod.xml`'s ramped and fixed collision-energy method entries
  across strides 216/220/284; not available for QQQ MRM records
  (strides 186/196), which don't use this field.

### Changed

- `crates/openaraw/src/raw/msperiodicactuals.rs`'s `parse()` no longer
  unconditionally returns an empty `Vec`. It now decodes the confirmed
  68-byte header / 20-byte record layout (see
  `docs/format/02-secondary-bins.md`) and populates `ActualsRecord`'s
  new `actual_id` field alongside `retention_time_min`, which is set for
  the first time. `Value` decoding (channel-dependent, requires
  `MSActualDefs.xml`) remains out of scope. As before, this parser has
  no caller in `Reader` - `openmassspec_core`'s output schema has no
  telemetry field - so it remains a raw-level API for downstream
  tooling rather than being wired into the spectrum pipeline.

### Documentation

- Documented why `PrecursorInfo::selected_mz` and
  `SpectrumRecord::polarity` are always `None`: both were investigated
  against the corpus (a mixed-polarity run in PXD031771, an exhaustive
  per-offset scan for a second precursor m/z field) and found
  unrecoverable from `MSScan.bin`'s current record layout, not just
  unimplemented. See `docs/format/06-known-limitations.md`.
- Documented why `msmasscal::parse` has no caller despite being a fully
  confirmed decoder: the coefficients it decodes are already applied to
  `MSPeak.bin`'s centroid data by the acquisition firmware, and
  `openmassspec_core`'s output schema has no field for per-scan
  calibration provenance metadata. Kept as a public raw-level API
  rather than removed.

## [0.1.1] - 2026-07-11

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
