---
sidebar_position: 1
---

# Overview

Agilent MassHunter `.d` is a directory, not a single file. The reader
targets the modern `AcqData` layout (the only shape found across the
338-file PRIDE validation corpus - no legacy ChemStation `DATA.MS`
directories were encountered):

```
SomeRun.d/
  AcqData/
    Contents.xml            - manifest of what's in this AcqData folder
    AcqMethod.xml            - acquisition method
    Devices.xml, DeviceConfigInfo.xml   - instrument/device config
    DefaultMassCal.xml        - default calibration
    MSActualDefs.xml           - field definitions for MSPeriodicActuals.bin
    MSScan.bin + MSScan.xsd    - per-scan index (see MSScan.bin)
    MSPeriodicActuals.bin      - instrument telemetry log
    MSProfile.bin              - profile-mode spectra (Q-TOF only, when present)
    MSPeak.bin                 - centroid/MRM spectra
    MSMassCal.bin               - per-scan calibration coefficients (Q-TOF only)
  Results/Qual/...            - post-processing outputs, not read by OpenARaw
```

All `.xml`/`.xsd` files are plain text, published alongside the binary
data by Agilent's own acquisition software - reading them is not reverse
engineering, and several of them (`MSScan.xsd`, `MSActualDefs.xml`,
`DefaultMassCal.xml`) directly document fields inside the neighboring
`.bin` files.

## Binary files

| File | Purpose | Status |
| --- | --- | --- |
| [MSScan.bin](./msscan) | Per-scan index: retention time, MS level, and byte offsets into the payload files | Confirmed |
| [MSPeak.bin](./mspeak) | Centroid (Q-TOF) or single-point MRM (QQQ) peak data | Confirmed |
| [MSProfile.bin](./msprofile) | Profile-mode intensity arrays, LZF-compressed | Confirmed |
| [MSPeriodicActuals.bin](./secondary-bins) | Instrument telemetry (pressures, temperatures, voltages, ...) | Confirmed |
| [MSMassCal.bin](./msmasscal) | Per-scan calibration coefficient traceability (Q-TOF only) | Confirmed |

See [Known limitations](./known-limitations) for the specific corpus
files and conditions the current reader does not handle.

## Clean-room provenance

Every byte-level claim on these pages came from binary analysis of the
public PRIDE corpus (see
[`CORPUS.md`](https://github.com/Sigilweaver/OpenARaw/blob/main/CORPUS.md))
plus the plain-text `.xml`/`.xsd` schema files Agilent ships alongside its
own data. No Agilent SDK, MassHunter software, or other vendor tooling was
used at any point - see
[`CONTRIBUTING.md`](https://github.com/Sigilweaver/OpenARaw/blob/main/CONTRIBUTING.md#vendor-software-and-clean-room-policy).
