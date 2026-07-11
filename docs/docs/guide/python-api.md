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
acquisitions, use the Rust reader's `iter_spectra` instead.

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
| `retention_time_sec`  | `float`       | Retention time in seconds            |

`len(spectrum)` returns the peak count. See [Scan data](./scan-data) for
how `mz`/`intensity` are derived from the on-disk profile and centroid
records.

```python
spectrum = reader.read_spectrum(0)
print(spectrum.ms_level, spectrum.retention_time_sec, len(spectrum))
mz, intensity = spectrum.mz, spectrum.intensity
```

## Next

- [Reader API](./reader) (Rust)
- [Scan data layouts](./scan-data)
