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

The writer checks every record and stops at the first one it rejects.
A Q-TOF MS/MS collision energy has no unit (the file does not state one,
see [MSScan.bin](../format/msscan)), and the reader declares that in
`RunMetadata::unstated`, so the bundle converts with default options. The
value is written as a unit-less `userParam`, each MS/MS spectrum carries an
`openmassspec: warning unknown_collision_energy_unit` entry, and the run
carries one `unstated_by_source` warning:

```rust
use openmassspec_core::write_mzml;

let summary = write_mzml(&mut reader, &mut out)?;
println!("{:?}", summary.warning_counts);
```

A spectrum with no peaks carries an `empty_spectrum` warning and declares
no base peak and no m/z range.

The writer also stops at the first spectrum that fails to decode and returns
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
