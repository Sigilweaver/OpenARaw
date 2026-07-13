# MSPeriodicActuals.bin

Binary telemetry log. Always present in `.d/AcqData/`. Records instrument diagnostic values (pressures, temperatures, voltages, charge states, etc.) at irregular intervals throughout the run. Field definitions are in the companion `MSActualDefs.xml`.

## Status: Confirmed

## Implementation Status

`crates/openaraw/src/raw/msperiodicactuals.rs` decodes `ActualID` and
`Time` (record offsets 4 and 8, constant across both variants below) into
`ActualsRecord`. It does not decode `Value` - that requires cross-
referencing the per-`ActualID` `DataType` from `MSActualDefs.xml` (see the
DataType table below), which this parser does not read. `parse` has no
caller in `Reader`/`SpectrumSource`: `openmassspec_core`'s output schema
has no field for instrument telemetry, so this stays a raw-level API for
downstream QC/provenance tooling rather than a stub - see the module docs
for the corpus retention-time cross-validation.

## File Layout

- **Global header:** 68 bytes.
- **Record stride:** 20 bytes (confirmed via file size arithmetic and sequential parsing on PXD004747 and PXD001310).

## Record Layout (20 bytes, little-endian)

The layout has two variants depending on whether the instrument is a QQQ or Q-TOF. The `ActualID` and `Time` fields are always at the same offsets; the `Value` field position shifts.

### QQQ variant (PXD004747, G6410A)

| Bytes | Field               | Type    |
|-------|---------------------|---------|
| 0-3   | OccurrenceFlag      | int32 (1 on first event for this ActualID in a block, else 0) |
| 4-7   | ActualID            | int32   |
| 8-15  | Time                | double (minutes) |
| 16-19 | Value               | float32 or int32 (see DataType) |

### Q-TOF variant (PXD001310, G6550A)

| Bytes | Field    | Type    |
|-------|----------|---------|
| 0-3   | Value    | float32 or int32 (see DataType) |
| 4-7   | ActualID | int32   |
| 8-15  | Time     | double (minutes) |
| 16-19 | Padding  | int32 (always 0, or int32 for DataType=3) |

## DataType Mapping (from MSActualDefs.xml)

All `DataType` codes observed across QQQ (PXD004747) and Q-TOF (PXD001310) files:

| DataType | Value Type | Value Offset (QQQ) | Value Offset (Q-TOF) | Examples |
|----------|------------|--------------------|-----------------------|---------|
| 3        | int32      | offset 16          | offset 16             | Charge State (ActualID=57), Min Range (ActualID=361) |
| 5        | float32    | offset 16          | offset 0              | Capillary Current (QQQ, ID=10), Funnel DC (Q-TOF, ID=85) |
| 6        | float32    | N/A (not in QQQ)   | offset 0              | Rough Vac (ID=29, ~0.52 Torr), Quad Vac (ID=30), Gas Temp (ID=27), Drying Gas (ID=34) |

- **DataType=5 QQQ** example: ActualID=10 (Capillary Current), value ~39 uA at offset 16.
- **DataType=5 Q-TOF** example: ActualID=85 (Funnel DC), value 4.76 V at offset 0.
- **DataType=6** example: ActualID=29 (Rough Vac), value 0.52 Torr at offset 0. Physically plausible for a rough vacuum pump.
- **DataType=3** example: ActualID=57 (Charge State), int values 0, 2, 3, 5, 7 at offset 16 (Q-TOF).

## Unresolved

- Whether the QQQ/Q-TOF variant distinction is instrument-model-specific or encoded somewhere in the file header.
- DataType values beyond 3, 5, 6 are not observed in the two files checked. The full set of DataType codes in the Agilent schema is not known from the XML alone.
