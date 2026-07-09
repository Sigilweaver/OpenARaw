# OpenARaw

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust MSRV](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

> Part of the [OpenProteo](https://sigilweaver.app/openproteo/docs/)
> stack for mass spectrometry raw-file access. Sibling readers:
> [OpenTFRaw](https://github.com/Sigilweaver/OpenTFRaw) (Thermo),
> [OpenWRaw](https://github.com/Sigilweaver/OpenWRaw) (Waters),
> [OpenTimsTDF](https://github.com/Sigilweaver/OpenTimsTDF) (Bruker).

Rust and Python reader for Agilent MassHunter `.d` mass spectrometry data
directories, clean-room reverse-engineered with no Agilent SDK or
software dependency. Covers the modern `AcqData` MassHunter shape across
Q-TOF (profile and centroid) and QQQ (MRM) acquisitions.

## Status

Not yet published to crates.io or PyPI. The reader is implemented and
validated against the full corpus (332 of 338 real-world PRIDE `.d`
datasets pass conformance checks end to end; the remaining 6 are
malformed source uploads, see
[docs/format/06-known-limitations.md](docs/format/06-known-limitations.md)).

## Install

Not yet published. Once released:

```sh
cargo add openaraw
```

```sh
pip install openaraw
```

## Quickstart

Rust:

```rust
use openaraw::reader::Reader;
use openproteo_core::SpectrumSource;

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
