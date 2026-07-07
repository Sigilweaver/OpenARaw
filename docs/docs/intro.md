---
sidebar_position: 1
slug: /
---

# OpenARaw

:::info Part of the OpenProteo stack

OpenARaw is one of the vendor readers in
[OpenProteo](https://sigilweaver.app/openproteo/docs/), a Rust- and
Python-native stack for proteomics raw-file access. Sibling readers:
[OpenTFRaw](https://sigilweaver.app/opentfraw/docs/) (Thermo `.raw`),
[OpenWRaw](https://sigilweaver.app/openwraw/docs/) (Waters `.raw/`),
[OpenTimsTDF](https://sigilweaver.app/opentimstdf/docs/) (Bruker `.d/`).

:::

OpenARaw is a Rust library that reads Agilent MassHunter `.d`
mass-spectrometry data directories - the directory-based binary format
produced by Agilent Q-TOF and QQQ instruments running MassHunter
acquisition software.

It runs on Linux, macOS, and Windows, with no dependency on any Agilent
SDK or software. The format was decoded by clean-room binary analysis of
a public corpus of mass-spectrometry datasets (PRIDE accessions); see
[`CORPUS.md`](https://github.com/Sigilweaver/OpenARaw/blob/main/CORPUS.md).

Optional Python bindings are available via the
[`openaraw`](./install) wheel.

## What it covers

| Component                                    | Status    |
| --------------------------------------------- | --------- |
| `MSScan.bin` (per-scan index)                 | supported |
| `MSPeak.bin` (centroid/MRM peak data)         | supported |
| `MSProfile.bin` (profile-mode data, LZF)      | supported |
| `MSPeriodicActuals.bin` (instrument telemetry)| documented, not yet exposed via the reader API |
| `MSMassCal.bin` (calibration traceability)    | documented, not required for reading spectra |
| Q-TOF Auto-MS/MS precursor m/z                | supported |
| QQQ MRM precursor mapping                     | supported |

Validated against 332 of 338 real-world PRIDE `.d` datasets end to end;
the remaining 6 are malformed source uploads, not format variants - see
[Known limitations](./format/known-limitations).

## Next steps

- [Install](./install) the Rust crate or the Python package.
- Run through the [Quickstart](./quickstart).
- Read the [Format specification](./format/overview) for the binary
  layer.

## License

OpenARaw is Apache-2.0 licensed. See [License](./license).
