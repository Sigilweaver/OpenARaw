---
sidebar_position: 3
---

# Instrument families

OpenARaw's corpus covers two Agilent instrument families, both under the
modern `AcqData` MassHunter directory layout:

## Q-TOF

Quadrupole time-of-flight instruments (models `G6540B`, `G6550A`
confirmed in the validation corpus). Q-TOF acquisitions can carry profile
data, centroid data, or both, and typically run Auto-MS/MS (data-dependent
acquisition): MS1 survey scans interleaved with MS2 scans against
automatically selected precursors.

- `analyzer` on yielded spectra is `Analyzer::TOFMS`.
- MS2 spectra carry a real `target_mz` precursor - see
  [Scan data](./scan-data).
- `MSMassCal.bin` is present, holding per-scan calibration coefficient
  traceability (already applied upstream - see
  [MSMassCal.bin](../format/msmasscal)).

## QQQ

Triple-quadrupole instruments (model `G6410A` confirmed in the validation
corpus) running targeted MRM (multiple reaction monitoring) acquisitions:
a fixed list of precursor/fragment transitions cycled repeatedly through
the run.

- `analyzer` on yielded spectra is `Analyzer::TQMS`.
- Every scan is effectively MS2; there is no isolation-window `target_mz`
  in the Q-TOF sense, so precursor identity is carried as
  `precursor_native_id` (`mrm_channel=<id>`) instead - see
  [Scan data](./scan-data).
- `MSMassCal.bin` is absent - QQQ instruments don't use TOF-style dynamic
  mass recalibration.

## How the reader tells them apart

The reader does not read the instrument model string. It infers the
family from `MSScan.bin`'s record stride (`>= 220` bytes indicates Q-TOF;
QQQ records use a shorter 186- or 196-byte stride) - see
[MSScan.bin](../format/msscan) for the full stride table. This heuristic
has held across every corpus file tested; if you encounter a `.d`
directory it misclassifies, please
[open an issue](https://github.com/Sigilweaver/OpenARaw/issues).
