# OpenARaw

[![CI](https://github.com/Sigilweaver/OpenARaw/actions/workflows/ci.yml/badge.svg)](https://github.com/Sigilweaver/OpenARaw/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/openaraw.svg)](https://crates.io/crates/openaraw)
[![PyPI](https://img.shields.io/pypi/v/openaraw.svg)](https://pypi.org/project/openaraw/)
[![docs.rs](https://img.shields.io/docsrs/openaraw)](https://docs.rs/openaraw)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust MSRV](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

> Part of the [OpenMassSpec](https://github.com/Sigilweaver/OpenMassSpec)
> stack for mass spectrometry raw-file access.

Rust and Python reader for Agilent MassHunter `.d` mass spectrometry data
directories, with no Agilent SDK or software dependency. Covers the
modern `AcqData` MassHunter shape across Q-TOF (profile and centroid)
and QQQ (MRM) acquisitions.

Documentation: [sigilweaver.app/openaraw/docs](https://sigilweaver.app/openaraw/docs)

## Known issues

These affect 0.2.0 and earlier. A fix is in progress.

- **Q-TOF m/z values are incorrect.** Q-TOF centroid x-values are
  uncalibrated flight times, and the reader reports them as m/z without
  applying the bundle's mass calibration (`DefaultMassCal.xml` /
  `MSMassCal.bin`). The error depends on m/z and runs to hundreds of Da,
  so Q-TOF m/z values from these releases should not be used.
- **Profile spectra are not emitted.** Profile blocks in `MSProfile.bin`
  fail to decode, and the reader silently falls back to centroid data
  for every scan.
- **QQQ (MRM) centroid data is unaffected.** MRM m/z values are stored
  directly and are read as-is.

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

Apache-2.0. See [LICENSE](LICENSE).

The format specification was developed by binary analysis of public
mass-spectrometry datasets (PRIDE accessions). See
[CORPUS.md](CORPUS.md) and [ATTRIBUTION.md](ATTRIBUTION.md).
