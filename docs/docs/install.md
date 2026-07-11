---
sidebar_position: 2
---

# Install

Most users should read Agilent data through the
[OpenMassSpec](https://github.com/Sigilweaver/OpenMassSpec) umbrella with
the `agilent` feature/extra, which adds format auto-detection, mzML
conversion, and Arrow streaming across every supported vendor:

```sh
cargo add openmassspec-io --features agilent
pip install openmassspec[agilent]
```

Use `openaraw` directly when you want the standalone Agilent reader with
minimal dependencies.

## Rust

```toml
[dependencies]
openaraw = "0.1"
```

OpenARaw needs Rust 1.85 or newer. There are no native or system
dependencies.

## Python

```sh
pip install openaraw
```

Or build from source (requires a Rust toolchain and `maturin`):

```sh
git clone https://github.com/Sigilweaver/OpenARaw
cd OpenARaw
maturin develop --release --manifest-path crates/openaraw-py/Cargo.toml
```

## Verifying the install

Rust:

```sh
cargo test --workspace
```

Python:

```python
import openaraw
reader = openaraw.RawReader("sample.d")
print(reader.scan_count)
```

## Optional: corpus fetcher

The validation corpus is not redistributed. It is pulled on demand from
the [PRIDE Archive](https://www.ebi.ac.uk/pride/) using local research
tooling (not part of the published crate):

```sh
python -m analysis.pride fetch <PXD_ACCESSION>
```

See [`CORPUS.md`](https://github.com/Sigilweaver/OpenARaw/blob/main/CORPUS.md)
for the file list and provenance.
