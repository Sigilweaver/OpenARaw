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

## Known issues

- **Q-TOF bundles with neither `MSMassCal.bin` nor `DefaultMassCal.xml`
  yield empty spectra.** There is no calibration to convert flight times
  to m/z. Each affected block is logged and counted
  (`Reader::centroid_decode_failures()`,
  `Reader::profile_decode_failures()`); flight times are never reported
  as m/z. One corpus bundle (`PXD031771/655_2.d`) is affected.

QQQ (MRM) m/z values are stored directly and are read as-is.

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

Apache-2.0. See [LICENSE](https://github.com/Sigilweaver/OpenARaw/blob/main/LICENSE).

The format specification was developed by binary analysis of public
mass-spectrometry datasets (PRIDE accessions). See
[CORPUS.md](https://github.com/Sigilweaver/OpenARaw/blob/main/CORPUS.md) and [ATTRIBUTION.md](https://github.com/Sigilweaver/OpenARaw/blob/main/ATTRIBUTION.md).
