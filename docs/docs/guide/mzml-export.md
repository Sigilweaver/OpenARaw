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

The writer checks every record and stops at the first one it rejects. A
Q-TOF MS/MS spectrum is rejected by default because its collision energy
has no unit (the file does not state one, see [MSScan.bin](../format/msscan)):

```text
invalid record "scanId=210655": collision energy is set but its unit (eV or NCE) is not known
```

Allow the check `unknown_collision_energy_unit` to write the value as a
unit-less `userParam` and put an `openmassspec: warning
unknown_collision_energy_unit` entry on each affected spectrum:

```sh
cargo run --release --example to_mzml -- sample.d output.mzML --allow unknown_collision_energy_unit
```

```rust
use openmassspec_core::{write_mzml_with_options, Check, Leniency, WriteOptions};

let opts = WriteOptions::new()
    .with_leniency(Leniency::strict().with(Check::UnknownCollisionEnergyUnit));
let summary = write_mzml_with_options(&mut reader, &mut out, &opts)?;
println!("{:?}", summary.warning_counts);
```

MS1-only and QQQ MRM bundles convert without the check.

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
