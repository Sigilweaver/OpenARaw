# OpenARaw

[![CI](https://github.com/Sigilweaver/OpenARaw/actions/workflows/ci.yml/badge.svg)](https://github.com/Sigilweaver/OpenARaw/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/openaraw.svg)](https://crates.io/crates/openaraw)
[![PyPI](https://img.shields.io/pypi/v/openaraw.svg)](https://pypi.org/project/openaraw/)
[![docs.rs](https://img.shields.io/docsrs/openaraw)](https://docs.rs/openaraw)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/Sigilweaver/OpenARaw/blob/main/LICENSE)
[![Rust MSRV](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org)

> Part of the [OpenMassSpec](https://github.com/Sigilweaver/OpenMassSpec)
> stack for mass spectrometry raw-file access.

Rust and Python reader for Agilent MassHunter `.d` mass spectrometry data
directories, with no Agilent SDK or software dependency. Covers the
modern `AcqData` MassHunter shape across Q-TOF (profile and centroid)
and QQQ (MRM) acquisitions.

Documentation: [sigilweaver.app/openaraw/docs](https://sigilweaver.app/openaraw/docs)

## Known issues and gaps

- **Q-TOF m/z accuracy depends on the run's calibration.** The reader
  applies each scan's stored calibration and does no reference-mass
  correction of its own. Measured on public corpus runs: peptide fragment
  ions (y1, immonium) read at a median of -2 to -9 ppm (95th percentile
  8 to 16 ppm), and background ions not used for locking (siloxanes,
  DEHP) at a median of -3.6 to +3.2 ppm over 19 bundles. Runs without
  reference-mass correction can be off by 10 to 20 ppm:
  - PXD031927 (G6550A): reference ion +12.5 ppm, siloxanes +17 to +19 ppm.
  - PXD044639 (G6550B, acquisition software 10.1): purine 121.0509 at
    -17.5 to -19.2 ppm.
  - PXD057471 (G6540A, `DefaultMassCal.xml` only): DEHP at -11 to -13.5
    ppm.
- **Stored MS2 precursor m/z can disagree with the MS1 peak.** In
  PXD031402 and PXD014285 the precursor target m/z differs from the
  matching MS1 peak by about 32 ppm and 10 ppm. Use a precursor tolerance
  of at least 30 ppm, or re-pick the precursor from the MS1 scan. Q-TOF
  MS2 spectra have no parent-scan link, charge state or isolation width.
- **Triple quadrupole (MRM) spectra report only the product m/z.** The
  precursor m/z is not set and per-transition SRM chromatograms are not
  emitted; each spectrum carries its MRM channel number
  (`precursor_native_id = "mrm_channel=N"`). Product m/z values are stored
  directly and match the method's transitions.
- **Bundles that cannot be decoded raise errors.** A Q-TOF bundle with
  neither `MSMassCal.bin` nor `DefaultMassCal.xml` has no calibration
  (`PXD031771/655_2.d`), and a bundle whose scan records point into a
  missing data file cannot be read (`PXD022473/M.d`, no `MSProfile.bin`).
  Each affected scan is an error from `iter_spectra` (`RuntimeError` in
  Python); flight times are never reported as m/z. Malformed folders
  without `AcqData` or `MSScan.bin` fail to open (6 of 338 corpus
  uploads).
- **Corpus gaps.** Tested instruments are the G6410A triple quadrupole
  and G6530, G6540 and G6550 Q-TOFs. Negative mode is covered only by
  the short negative segment of five PXD031771 runs (190 spectra). Not
  covered: ion mobility (6560), GC-MS ChemStation `.D` folders
  (`DATA.MS`), and models not yet seen in public data (for example 6545,
  6546, 6470 and 6495).

## Install

**Prefer [`openmassspec-io`](https://github.com/Sigilweaver/OpenMassSpec)
with the `agilent` feature/extra** unless you need this parser standalone
(minimal dependencies, or building your own abstraction) - the umbrella
gives you format auto-detection, mzML conversion, and Arrow streaming
across all wired-in vendors for free:

```sh
cargo add openmassspec-io --features agilent
```

```sh
pip install openmassspec[agilent]
```

Standalone:

Rust:

```sh
cargo add openaraw
```

Python:

```sh
pip install openaraw
```

## Quickstart

Rust:

```rust
use openaraw::reader::Reader;
use openmassspec_core::SpectrumSource;

let mut reader = Reader::open("sample.d")?;
for spectrum in reader.iter_spectra() {
    let spectrum = spectrum?;
    println!("{}: {} peaks", spectrum.native_id, spectrum.mz.len());
}
```

Python:

```python
import openaraw

reader = openaraw.RawReader("sample.d")
spectrum = reader.read_spectrum(0)
print(spectrum.ms_level, spectrum.retention_time_sec, len(spectrum.mz))
```

See the [docs site](https://sigilweaver.app/openaraw/docs) for the full
guide, format specification, and API reference.

## License

Apache-2.0. See [LICENSE](https://github.com/Sigilweaver/OpenARaw/blob/main/LICENSE).

The format specification was developed by binary analysis of public
mass-spectrometry datasets (PRIDE accessions). See
[CORPUS.md](https://github.com/Sigilweaver/OpenARaw/blob/main/CORPUS.md) and [ATTRIBUTION.md](https://github.com/Sigilweaver/OpenARaw/blob/main/ATTRIBUTION.md).
