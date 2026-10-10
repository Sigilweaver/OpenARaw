# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

This release changes the public Rust API (see Changed and Removed) and
ships as 0.3.0.

### Added

- `Reader::profile_decode_failures()` and
  `Reader::centroid_decode_failures()` count spectrum payload blocks that
  failed to read or decode. Failures are logged through the `log` crate
  (first per kind at warn, the rest at debug) instead of being dropped
  silently. New dependency: `log` 0.4.
- Q-TOF flight-time calibration. Centroid x-values and the profile axis
  are converted with `m/z = (A * (t - T0))^2 - P(clamp(t, t_min, t_max))`,
  where the `Polynomial` step's powers are the set bits of its
  `ValueUseFlags` in `DefaultMassCal.xml`. Each scan uses its own
  `MSMassCal.bin` record, or the `DefaultMassCal.xml` entry for its
  `CalibrationID` when the bundle has no `MSMassCal.bin` (35 of 224
  corpus Q-TOF bundles). Reproduces the stored `BasePeakMZ` across the
  corpus. New public items: `raw::calibration` (`TofCalibration`,
  `PolynomialStep`, `CalibrationSource`), `raw::defaultmasscal`,
  `raw::msmasscal::parse_bytes`, `raw::msscan::is_qtof_stride`,
  `raw::msprofile::ProfileAxis`, `Reader::calibration()`.
- `ScanRecord::tic`, `ScanRecord::base_peak_mz` and
  `ScanRecord::calibration_id` (`MSScan.bin` offsets 28, 36, 52) for
  Q-TOF records; spectra now carry `total_ion_current` and
  `base_peak_mz`, and `extra["openaraw.calibration_id"]`. The Python
  `scan_index()` records expose the same fields plus
  `calibration_source`.
- `qtof_calibration_check` example and corpus test
  (`tests/qtof_calibration.rs`) measuring decoded base peak m/z against
  the stored `BasePeakMZ` (median / p95 ppm), summed intensity against
  `TIC`, and reference-mass ions.
- `OPENARAW_CORPUS` and `REQUIRE_CORPUS=1` environment variables for
  corpus-backed tests and the `corpus_scan` example, replacing hardcoded
  `/workspaces/...` paths. CI now fails if a downloaded fixture is
  missing instead of skipping.

### Changed

- Breaking: `raw::mspeak::decode_peak_block` takes an
  `Option<&TofCalibration>` (required for Q-TOF blocks), and
  `raw::msprofile::decode_profile_block` takes a `&TofCalibration`
  instead of `min_x` / `max_x`.
- Breaking: `raw::msscan::ScanRecord` has new public fields (`tic`,
  `base_peak_mz`, `calibration_id`), so struct literals must set them.
- Breaking: `raw::msmasscal::parse` returns an error for a file shorter
  than its header, a partial trailing record or an oversized coefficient
  count, instead of an empty or truncated list.
- Minimum supported Rust version is now 1.88, matching
  `openmassspec-core` 2.0.0.

### Removed

- Breaking: `ScanRecord::unidentified_offset_36` and the
  `openaraw.msscan_offset36_unidentified` spectrum extra; the value is
  now `ScanRecord::base_peak_mz`.
- Unused public `reader::DecodedSpectrum` enum. Technically a breaking
  API removal; nothing constructed or consumed it.

### Fixed

- Q-TOF centroid m/z is no longer the raw flight time divided by 100,
  and Q-TOF payloads without a usable calibration are counted as decode
  failures instead of being reported as m/z.
- Profile blocks: the LZF stream starts at byte 0 of the block and
  decompresses to an `f64` start time and `f64` sampling period followed
  by the intensities; the old 16-byte-preamble layout failed on every
  real block.
- `RunMetadata` now reports `MS:1001509` (Agilent MassHunter format) and
  `MS:1001508` (Agilent MassHunter nativeID format). It previously used
  `MS:1002846` and `MS:1002848`, which are PRIDE file-URI terms.
- Q-TOF MS2 `precursor_native_id` no longer points at the spectrum's own
  scan; it is `None` until the parent scan (DDScanID) is decoded.
- `MSScan.bin` stride detection no longer overflows when the first
  ScanID is near `u32::MAX`.
- `Devices.xml` and `Contents.xml` read errors (other than a missing
  file) and XML parse errors are logged at warn level instead of being
  ignored.
- Docs: `IonPolarity` is at `MSScan.bin` record offset 64 (per the
  bundle's `MSScan.xsd`, checked on PXD031771); the docs said polarity
  was not recoverable.
- Docs: README "Known issues" and format doc status lines for Q-TOF m/z
  and profile decoding, which the docs marked as confirmed.
- Docs: `MSPeak.bin` Q-TOF x-values are flight times (not `m/z * 100`),
  `MSMassCal.bin` coefficients are not pre-applied, the `MSProfile.bin`
  block layout is corrected, `MSScan.bin` offsets 28/36/52 are
  `TIC`/`BasePeakMZ`/`CalibrationID`, and the calibration formula and
  `DefaultMassCal.xml` layout are documented.

## [0.2.0] - 2026-09-28

### Added

- Python spectra and run metadata now expose every shared record field.
  Added bounded iteration, chromatograms, MSScan index data, and device
  identity. Namespaced extras preserve scan parameters and device details.

### Changed

- Breaking: `raw::mspeak::decode_peak_block` takes an
  `Option<&TofCalibration>` (required for Q-TOF blocks), and
  `raw::msprofile::decode_profile_block` takes a `&TofCalibration`
  instead of `min_x` / `max_x`.
- Breaking: `raw::msscan::ScanRecord` has new public fields (`tic`,
  `base_peak_mz`, `calibration_id`), so struct literals must set them.
- Breaking: `raw::msmasscal::parse` returns an error for a file shorter
  than its header, a partial trailing record or an oversized coefficient
  count, instead of an empty or truncated list.
- Adopts `openmassspec-core` 2.0.0 (arrow 60). Rust users building against
  core 1.x must upgrade core too.

### Fixed

- Stride-284 `MSScan.bin` record offset 36 is now decoded into
  `ScanRecord::unidentified_offset_36` and, when present, surfaced on each
  spectrum via `extra["openaraw.msscan_offset36_unidentified"]`. Its
  semantics remain unconfirmed - corpus comparison against PXD001310 rules
  out both decoded TIC and the max-intensity centroid's m/z, so it is
  deliberately not mapped to `total_ion_current` or `base_peak_mz`. This
  closes the "never parsed" half of #19; identifying the quantity itself
  still needs a fixture or repo-owned calibration path and stays open.

## [0.1.7] - 2026-08-12

### Changed

- Breaking: `raw::mspeak::decode_peak_block` takes an
  `Option<&TofCalibration>` (required for Q-TOF blocks), and
  `raw::msprofile::decode_profile_block` takes a `&TofCalibration`
  instead of `min_x` / `max_x`.
- Breaking: `raw::msscan::ScanRecord` has new public fields (`tic`,
  `base_peak_mz`, `calibration_id`), so struct literals must set them.
- Breaking: `raw::msmasscal::parse` returns an error for a file shorter
  than its header, a partial trailing record or an oversized coefficient
  count, instead of an empty or truncated list.
- Adopt `openmassspec-core` 1.5.0. The new `SpectrumRecord`/`RunMetadata`
  `extra` and `acquisition_event_id` fields are defaulted (empty / `None`);
  no field is populated yet, so output is unchanged.

### Fixed

- Stride-284 `MSScan.bin` records now expose their instrument-reported
  base-peak intensity from offset 44 on each spectrum. Corpus comparison
  against OpenARaw's decoded centroid arrays confirmed that mapping and ruled
  out TIC; offset 36 remains documented but intentionally unwired because its
  interpretation could not be verified from repo-owned evidence. Refs #19.

### Testing

- CI now also downloads a small QTOF `.d.zip` fixture from PRIDE
  (PXD030293/180814-Sample19.d, ~3 MB unpacked, ~960 KB zipped, an Agilent
  6550 acquisition per `AcqData/Devices.xml`'s `ModelNumber`) and unzips it
  ahead of `cargo test`, so `test_qtof_conformance` now exercises a real
  decode path in CI instead of always skipping. This replaces the
  previously-known smallest QTOF candidate
  (PXD004426/20140806_TgAAL.d, ~247 MB zipped - see CORPUS.md), which
  remains as a local-dev-only fallback in `qtof_fixture()`. Closes #28.

## [0.1.6] - 2026-07-29

### Fixed

- Adapted `RunMetadata` construction to `openmassspec-core` 1.4.0's new
  `acquisition_software_name`/`acquisition_software_version` fields
  (defaulted to `None`; nothing in `Devices.xml` or `Contents.xml` is
  currently parsed into a reusable acquisition-software string - see
  `docs/format/01-msscan.md`'s `AcqSoftwareVersion` note - so wiring up a
  real value is left as future work rather than done here). Closes #26.
- `Reader`'s per-spectrum `analyzer` field (TOFMS vs TQMS) was inferred
  purely from `MSScan.bin`'s record stride (`stride >= 220` => Q-TOF), a
  heuristic standing in for a real parse. It is now resolved from
  `Devices.xml`'s `<Name>` field (`"QTOF"`/`"TandemQuadrupole"`, confirmed
  across the 330-bundle validation corpus) the same way `resolve_instrument`
  already resolves the run's instrument CV term, falling back to the
  stride guess only when `Devices.xml` is missing, unparseable, or names a
  device outside those two known values. `SpectrumRecord::polarity` was
  also investigated per the issue (whether it's recoverable from a
  metadata stream such as `Contents.xml` or an acquisition-method file
  rather than per-scan binary data); the existing investigation in
  `docs/format/06-known-limitations.md` already covers this ground (both
  `AcqMethod.xml`'s per-segment `ionPolarity` and
  `MSPeriodicActuals.bin`'s `ActualID=65` channel were checked against a
  known mixed-polarity run and found inconclusive/unusable), so it remains
  documented there as a known limitation rather than left as an open
  question. Fixes #22. (@Nabejo)

### Added

- `Reader::iter_chromatograms` (Sigilweaver/OpenARaw#17): overrides the
  `SpectrumSource` trait default so TIC and base-peak chromatograms reach
  mzML output through OpenMassSpecCore's `<chromatogramList>` writer. Both
  traces are derived from already-decoded MS1 spectra - TIC from the summed
  scan intensities (`MS:1000235` "total ion current chromatogram") and BPC
  from each scan's most intense point (`MS:1000628` "basepeak chromatogram")
  - so they stay self-consistent with the emitted spectra and require no new
  binary parsing. Runs with no MS1 scans (e.g. QQQ MRM) emit no summary
  chromatograms. SRM/MRM transition chromatograms are not produced because
  the Q1/Q3 isolation windows they need are not decoded (only an opaque
  `mrm_channel_id` is). (@Nabejo)

### Testing

- CI now downloads a small QQQ (MRM) `.d.zip` fixture from PRIDE
  (PXD004747/Cdc19_ubp2_AQUA.d, ~9 MB unpacked) and unzips it ahead of
  `cargo test`, so `test_qqq_conformance` exercises a real decode path in
  CI instead of always skipping, matching the corpus-download pattern
  used by the other vendor crates (e.g. OpenTFRaw). `test_qtof_conformance`
  still skips in CI: no QTOF bundle small enough for a CI download has
  turned up yet (the smallest known one is ~247 MB zipped - see
  CORPUS.md). Fixes #20. (@Nabejo)

### Documentation

- Updates documentation for Sigilweaver/OpenARaw#21: documented why
  `SpectrumRecord::inv_mobility`, `SpectrumRecord::inv_mobility_per_peak`,
  and `RunMetadata::mobility_array_kind` are always `None`. Unlike the
  polarity/`selected_mz` gaps
  above, this isn't a field that was searched for and not found - no
  ion-mobility acquisition (e.g. from an Agilent 6560 IM-QTOF) exists
  anywhere in the 338-bundle PRIDE validation corpus to reverse-engineer
  a drift-time layout against in the first place. Implementing this
  without a real sample to check against would mean guessing at a file
  name and record layout, which the project's clean-room policy
  prohibits. See `docs/format/06-known-limitations.md` for the open
  questions (record layout vs. separate frame file, CCS vs. raw drift
  time) to resolve once a real 6560 bundle is available. (@Nabejo)

## [0.1.5] - 2026-07-25

### Fixed

- Declared `openmassspec-core` minimum was still `"1.0.0"` even after
  0.1.4 started requiring 1.3.0's new `RunMetadata` fields to compile;
  bumped the declared minimum to `"1.3.0"` to match.

## [0.1.4] - 2026-07-25

### Fixed

- Adapted `RunMetadata` construction to `openmassspec-core` 1.3.0's new
  `analyzers`/`instrument_serial_number` fields (defaulted, as neither is
  decoded here).

## [0.1.3] - 2026-07-20

### Security

- `read_bytes` (used to load `MSProfile.bin`/`MSPeak.bin` blocks by the
  `SpectrumOffset`/`ByteCount` a `MSScan.bin` record supplies) now checks
  the requested length against the actual remaining file size before
  allocating, instead of allocating first. A crafted `ByteCount` could
  previously force an allocation up to ~4 GB regardless of the real file
  size.
- `lzf::decompress` now caps its `UncompressedByteCount`-driven
  allocation at 512 MiB before allocating, for the same reason: that
  field is a file-controlled `u32` with no independent bound (unlike
  `ByteCount`, decompressed size isn't checkable against the file size
  directly). 512 MiB is well beyond any real Q-TOF profile spectrum.
  Fixes #1. (@Nabejo)

### Testing

- Added synthetic byte-slice unit tests for the block decoders (`lzf`,
  `msscan`, `mspeak`, `msprofile`), which previously had no coverage on
  CI (the only tests were corpus-gated and skip when the corpus is
  absent). Covers LZF literal/back-reference decoding and its malformed-
  input error paths (including the new allocation cap above), MSScan.bin
  stride detection across all five candidate strides plus a regression
  test for the short-payload bounds guard (0.1.1), and minimal
  MSPeak/MSProfile record decoding. `MSScan` gained a `from_bytes`
  associated function (`from_path` now delegates to it) so the parser
  can be exercised without touching the filesystem.
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

- Breaking: `raw::mspeak::decode_peak_block` takes an
  `Option<&TofCalibration>` (required for Q-TOF blocks), and
  `raw::msprofile::decode_profile_block` takes a `&TofCalibration`
  instead of `min_x` / `max_x`.
- Breaking: `raw::msscan::ScanRecord` has new public fields (`tic`,
  `base_peak_mz`, `calibration_id`), so struct literals must set them.
- Breaking: `raw::msmasscal::parse` returns an error for a file shorter
  than its header, a partial trailing record or an oversized coefficient
  count, instead of an empty or truncated list.
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
