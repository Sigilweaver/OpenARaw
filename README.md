# OpenARaw

[![CI](https://github.com/Sigilweaver/OpenARaw/actions/workflows/ci.yml/badge.svg)](https://github.com/Sigilweaver/OpenARaw/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/openaraw.svg)](https://crates.io/crates/openaraw)
[![PyPI](https://img.shields.io/pypi/v/openaraw.svg)](https://pypi.org/project/openaraw/)
[![docs.rs](https://img.shields.io/docsrs/openaraw)](https://docs.rs/openaraw)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust MSRV](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

> Part of the [OpenMassSpec](https://sigilweaver.app/openmassspec/docs/)
> stack for mass spectrometry raw-file access. Sibling readers:
> [OpenTFRaw](https://github.com/Sigilweaver/OpenTFRaw) (Thermo),
> [OpenWRaw](https://github.com/Sigilweaver/OpenWRaw) (Waters),
> [OpenTimsTDF](https://github.com/Sigilweaver/OpenTimsTDF) (Bruker).

Rust and Python reader for Agilent MassHunter `.d` mass spectrometry data
directories, clean-room reverse-engineered with no Agilent SDK or
software dependency. Covers the modern `AcqData` MassHunter shape across
Q-TOF (profile and centroid) and QQQ (MRM) acquisitions.

Documentation: [sigilweaver.app/openaraw/docs](https://sigilweaver.app/openaraw/docs)

## Status

Published on crates.io (`openaraw`) and PyPI (`openaraw`). The reader is
validated against the full corpus (332 of 338 real-world PRIDE `.d`
datasets pass conformance checks end to end; the remaining 6 are
malformed source uploads, see
[docs/format/06-known-limitations.md](docs/format/06-known-limitations.md)).

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

## License

Apache-2.0. See [LICENSE](LICENSE).

The format specification was developed by binary analysis of public
mass-spectrometry datasets (PRIDE accessions). See
[CORPUS.md](CORPUS.md) and [ATTRIBUTION.md](ATTRIBUTION.md).
