---
sidebar_position: 4
---

# mzML export

OpenARaw writes mzML via `openproteo_core::write_mzml`, the same writer
every reader in the OpenProteo stack uses, so output is consistent across
vendors.

```sh
cargo run --release --example to_mzml -- path/to/sample.d output.mzML
```

Or from your own code:

```rust
use openaraw::reader::Reader;
use openproteo_core::write_mzml;

let mut reader = Reader::open("sample.d")?;
let mut out = std::fs::File::create("output.mzML")?;
write_mzml(&mut reader, &mut out)?;
```

`write_mzml` iterates the reader's spectra via `SpectrumSource` (the same
stream described in [Reader](./reader) and [Scan data](./scan-data)) and
emits PSI-MS CV-annotated mzML - `MS:1002846` (Agilent MassHunter format)
as the source-file format term, with per-spectrum polarity, scan mode,
analyzer, and precursor information carried through where the reader
populated it.
