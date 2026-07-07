---
sidebar_position: 2
---

# Install

OpenARaw is not yet published to crates.io or PyPI. Until then, use it
from source.

## Rust

```toml
[dependencies]
openaraw = { git = "https://github.com/Sigilweaver/OpenARaw" }
```

OpenARaw needs Rust 1.85 or newer. There are no native or system
dependencies.

## Python

From source (requires a Rust toolchain and `maturin`):

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
