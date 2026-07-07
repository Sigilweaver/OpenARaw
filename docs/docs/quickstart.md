---
sidebar_position: 3
---

# Quickstart

## Rust

```rust
use openaraw::reader::Reader;
use openproteo_core::SpectrumSource;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = Reader::open("sample.d")?;
    for spectrum in reader.iter_spectra() {
        println!("{}: {} peaks", spectrum.native_id, spectrum.mz.len());
    }
    Ok(())
}
```

The repo ships two examples:

```sh
cargo run --release --example to_mzml -- path/to/sample.d output.mzML
cargo run --release --example corpus_scan
```

`to_mzml` writes a minimal mzML file; `corpus_scan` runs the full
conformance suite against every `.d` directory in a corpus index (used to
validate the reader against all 338 corpus files, see
[Known limitations](./format/known-limitations)).

## Python

```python
import openaraw

reader = openaraw.RawReader("sample.d")
spectrum = reader.read_spectrum(0)
print(spectrum.ms_level, spectrum.retention_time_sec, len(spectrum.mz))
```

## Next

- [Reader API](./guide/reader)
- [Scan data](./guide/scan-data)
- [Instrument families](./guide/instrument-families)
- [Format specification](./format/overview)
