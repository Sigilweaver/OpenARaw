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

`Reader` implements `openproteo_core::SpectrumSource`, the shared trait
every vendor reader in the OpenProteo stack implements:

```rust
use openproteo_core::SpectrumSource;

let metadata = reader.run_metadata();
let scan_count = reader.spectrum_count_hint().unwrap_or(0);
println!("{} ({} scans)", metadata.source_file_name, scan_count);

let mut reader = reader;
for spectrum in reader.iter_spectra() {
    println!("{}\t{}\t{} peaks", spectrum.native_id, spectrum.ms_level, spectrum.mz.len());
}
```

Each yielded `SpectrumRecord` carries `index`, `scan_number`, `native_id`,
`ms_level`, `scan_mode` (`Centroid` or `Profile`), `analyzer` (`TOFMS` for
Q-TOF, `TQMS` for QQQ), `retention_time_sec`, `mz`/`intensity` arrays, and
- for MS2+ scans - a `precursor` (`PrecursorInfo`) populated either from
the Q-TOF isolation target m/z or the QQQ MRM channel ID. See
[Scan data](./scan-data) for how each field is derived from the on-disk
records.

## Error handling

Public functions return `openaraw::Result<T>`. The error type is
`openaraw::Error`, which wraps the failure category (`Io`, `Parse`) and a
message.
