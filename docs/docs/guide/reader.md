---
sidebar_position: 1
---

# Reader

The entry point is `Reader`. Opening a `.d` directory parses `MSScan.bin`
(the per-scan index) up front; individual scan payloads are read on
demand from `MSPeak.bin` / `MSProfile.bin` as you iterate.

```rust
use openaraw::reader::Reader;

let reader = Reader::open("sample.d")?;
```

`Reader::open` fails if the directory does not contain an `AcqData`
subdirectory (the modern MassHunter layout this reader targets - see
[Format specification](../format/overview)).

`Reader` implements `openmassspec_core::SpectrumSource`, the shared trait
every vendor reader in the OpenMassSpec stack implements:

```rust
use openmassspec_core::SpectrumSource;

let metadata = reader.run_metadata();
let scan_count = reader.spectrum_count_hint().unwrap_or(0);
println!("{} ({} scans)", metadata.source_file_name, scan_count);

let mut reader = reader;
for spectrum in reader.iter_spectra() {
    let spectrum = spectrum?;
    println!("{}\t{}\t{} peaks", spectrum.native_id, spectrum.ms_level, spectrum.mz.len());
}
```

`iter_spectra` yields one `openmassspec_core::Result<SpectrumRecord>` per
`MSScan.bin` record, in file order. A scan whose payload block cannot be
read or decoded is yielded as an `Err`, and iteration continues with the
next scan.

Each `SpectrumRecord` carries `index`, `scan_number`, `native_id`,
`ms_level`, `scan_mode` (`Centroid` or `Profile`), `analyzer` (`TOFMS` for
Q-TOF, `TQMS` for QQQ), `retention_time_sec`, `mz`/`intensity` arrays, and
- for MS2+ scans - a `precursor` (`PrecursorInfo`) populated either from
the Q-TOF isolation target m/z or the QQQ MRM channel ID. See
[Scan data](./scan-data) for how each field is derived from the on-disk
records.

## Error handling

Public functions return `openaraw::Result<T>`. The error type is
`openaraw::Error`: `Io`, `Parse` (a message), or `Scan` (a scan's payload
block failed: its `scan_id`, which `block`, and the underlying error).
The enum is `#[non_exhaustive]`, so match it with a wildcard arm.

`SpectrumSource` methods return `openmassspec_core::Error`. An
`openaraw::Error` converts into it with `From` (I/O errors become
`Error::Io`, the rest `Error::Vendor`, which can be downcast back to
`openaraw::Error`).

The reader also exposes `dir()`, `bundle_name()`, `msscan()` (the decoded
`MSScan.bin` records), `device()` (the `Devices.xml` mass spectrometer
entry) and `calibration(i)`. `profile_decode_failures()` and
`centroid_decode_failures()` count the scans yielded as errors so far.
