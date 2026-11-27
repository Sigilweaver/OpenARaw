---
sidebar_position: 5
---

# Python API

The `openaraw` wheel exposes a small, eager reader built on the same Rust
core as the [Rust reader](./reader). Install it with `pip install openaraw`
(or `pip install openmassspec[agilent]` for the umbrella).

```python
import openaraw

reader = openaraw.RawReader("sample.d")
```

Opening a `RawReader` parses the `.d` directory and decodes **every**
spectrum up front into memory, so construction is where the work happens
and subsequent access is cheap. For streaming access over very large
acquisitions, use `openaraw.iter_spectra(path)`, which decodes one
spectrum at a time.

A scan that cannot be decoded raises `RuntimeError`: from the
`RawReader` constructor, from `read_chromatograms()`, or from the
`iter_spectra` iterator when it reaches that scan.

## `RawReader`

| Member                 | Type            | Description                                              |
| ---------------------- | --------------- | ------------------------------------------------------- |
| `RawReader(path)`      | constructor     | Open and fully decode the `.d` directory at `path`      |
| `scan_count`           | `int`           | Number of decoded spectra                               |
| `read_spectrum(index)` | `Spectrum`      | The spectrum at zero-based `index` (raises if out of range) |

```python
print(reader.scan_count)
for i in range(reader.scan_count):
    spectrum = reader.read_spectrum(i)
    ...
```

## `Spectrum`

| Attribute             | Type          | Description                          |
| --------------------- | ------------- | ------------------------------------ |
| `mz`                  | `list[float]` | m/z values (float64)                 |
| `intensity`           | `list[float]` | Intensities (float32)                |
| `ms_level`            | `int`         | MS level (1 for MS1, 2+ for MS/MS)   |
| `retention_time_sec`  | `float \| None` | Retention time in seconds (always set by this reader) |
| `scan_mode`           | `str \| None` | `"profile"`, `"centroid"`, or `None` for a scan with no payload block |
| `analyzer`            | `str \| None` | `"tof"` (Q-TOF) or `"tqms"` (QQQ)    |
| `base_peak_mz`, `base_peak_intensity` | `float \| None` | Base peak of the decoded peaks; `None` when the spectrum has no peaks |
| `low_mz`, `high_mz`   | `float \| None` | Lowest and highest m/z of the spectrum, as the scan record gives them; `None` when the spectrum has no peaks |
| `reported_base_peak_mz`, `reported_base_peak_intensity` | `float \| None` | The base peak the scan record reported, also on a spectrum with no peaks |
| `precursor`           | `dict \| None` | MS2+ only, see below                 |
| `extra`               | `dict[str, str]` | Reader-specific values, such as `openaraw.mrm_channel_id` |

A spectrum with no peaks (`len(spectrum) == 0`) declares no base peak and no
m/z range: `base_peak_mz`, `base_peak_intensity`, `low_mz` and `high_mz` are
`None`. The `reported_*` attributes still return what the scan record held
(for example `reported_base_peak_mz` of 0.0). A range the record reported is kept as
strings in `extra["openaraw.reported_low_mz"]` and
`extra["openaraw.reported_high_mz"]`.

`len(spectrum)` returns the peak count. See [Scan data](./scan-data) for
how `mz`/`intensity` are derived from the on-disk profile and centroid
records.

```python
spectrum = reader.read_spectrum(0)
print(spectrum.ms_level, spectrum.retention_time_sec, len(spectrum))
mz, intensity = spectrum.mz, spectrum.intensity
```

### `precursor`

For MS2+ spectra `precursor` is a dict with `target_mz`, `selected_mz`,
`isolation_width`, `charge`, `intensity`, `collision_energies`,
`precursor_native_id`, `activation`, `analyzer` and `ccs`. Q-TOF MS/MS
scans set `target_mz` and one collision energy; QQQ MRM scans set neither
(the channel is in `spectrum.extra["openaraw.mrm_channel_id"]`).

`collision_energies` is a list of `{"value": float, "unit": str | None}`.
The file does not state the unit, so `unit` is `None`:

```python
>>> spectrum.precursor["collision_energies"]
[{'value': 13.15, 'unit': None}]
```

## Next

- [Reader API](./reader) (Rust)
- [Scan data layouts](./scan-data)
