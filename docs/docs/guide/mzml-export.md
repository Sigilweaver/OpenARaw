---
sidebar_position: 4
---

# mzML export

OpenARaw writes mzML via `openmassspec_core::write_mzml`, the same writer
every reader in the OpenMassSpec stack uses, so output is consistent across
vendors.

```sh
cargo run --release --example to_mzml -- path/to/sample.d output.mzML
```

Or from your own code:

```rust
use openaraw::reader::Reader;
use openmassspec_core::write_mzml;

let mut reader = Reader::open("sample.d")?;
let mut out = std::fs::File::create("output.mzML")?;
write_mzml(&mut reader, &mut out)?;
```

The writer stops at the first spectrum that fails to decode and returns
its error. To write the decodable spectra of a damaged bundle instead,
pass `RecordErrorPolicy::Skip` and check the returned summary:

```rust
use openmassspec_core::{write_mzml_with_options, RecordErrorPolicy, WriteOptions};

let opts = WriteOptions::new().with_record_error_policy(RecordErrorPolicy::Skip);
let summary = write_mzml_with_options(&mut reader, &mut out, &opts)?;
if summary.skipped() > 0 {
    eprintln!("{} spectra could not be decoded and were skipped", summary.skipped());
}
```

`write_mzml` iterates the reader's spectra via `SpectrumSource` (the same
stream described in [Reader](./reader) and [Scan data](./scan-data)) and
emits PSI-MS CV-annotated mzML - `MS:1001509` (Agilent MassHunter format)
as the source-file format term and `MS:1001508` (Agilent MassHunter
nativeID format) as the native ID format, with per-spectrum polarity, scan mode,
analyzer, and precursor information carried through where the reader
populated it.
