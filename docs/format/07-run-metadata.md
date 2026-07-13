# Run Metadata: Contents.xml and Devices.xml

Two small XML sidecars in `AcqData/` carry run-level metadata that isn't
present anywhere in the binary files: the acquisition start time and the
real instrument identity (vendor model number, not just a Q-TOF/QQQ
family guess).

## Status: Confirmed

Both files were surveyed across the full 330-bundle corpus subset that
has them (see `CORPUS.md`; 4 of the 334 non-malformed bundles are
missing one or both, see [06-known-limitations.md](06-known-limitations.md)).

## Contents.xml: `<AcquiredTime>`

```xml
<Contents ...>
  <Version>3</Version>
  <AcquiredTime>2014-08-06T22:49:56Z</AcquiredTime>
  ...
</Contents>
```

`<AcquiredTime>` is a single run-level acquisition start timestamp.
Observed corpus values are already RFC 3339-conformant, in two forms:

- `2014-08-06T22:49:56Z` (whole seconds, `Z` suffix) - the common case.
- `2021-11-30T18:36:39.6755756-07:00` (fractional seconds at .NET's
  100ns tick resolution, explicit UTC offset) - seen in a minority of
  bundles, notably PXD038602, PXD027071, PXD049393, and PXD031927.

Both are valid RFC 3339 as-is, so the reader passes the trimmed text
through unmodified rather than reformatting it.

`<InstrumentName>` also appears in `Contents.xml` (and is duplicated in
`sample_info.xml`'s `InstrumentName` field), but in every corpus file
it is the operator-assigned label `"Instrument 1"` - not useful for
instrument identification. Use `Devices.xml` instead (below).

## Devices.xml: the mass spectrometer's `<Device>` entry

```xml
<Devices ...>
  <Version>1</Version>
  <Device DeviceID="1">
    <Name>QTOF</Name>
    <ModelNumber>G6550A</ModelNumber>
    <SerialNumber>PP10000000</SerialNumber>
    <Type>6</Type>
    <StoredDataType>8</StoredDataType>
    ...
  </Device>
  <Device DeviceID="2">
    <Name>LowflowPump</Name>
    ...
  </Device>
  ...
</Devices>
```

`Devices.xml` lists every hardware device attached to the acquisition
(mass spectrometer, LC pumps, autosampler, column oven). **The mass
spectrometer is always the first `<Device>` element in document order**,
confirmed across all 330 corpus `Devices.xml` files with zero exceptions:

| First-device `Name`  | `Type` | `StoredDataType` | `ModelNumber`s observed                              | Count |
|-----------------------|--------|-------------------|-------------------------------------------------------|-------|
| `QTOF`                | `6`    | `8`               | `G6530A`, `G6530B`, `G6540A`, `G6540B`, `G6550A`, `G6550B` | 222 |
| `TandemQuadrupole`    | `5`    | `8`               | `G6410A`                                               | 108 |

Non-MS devices (pumps, autosamplers, column ovens) always have
`StoredDataType=2` and `Type` values outside `{5, 6}` (e.g. `21`-`40`
range for various pump/sampler/oven models). `DeviceID` numbering is
**not** reliable as a discriminator by itself - it starts at 1 in most
files but some bundles (e.g. PXD011212, PXD012013) number the MS device
`DeviceID="1"` while non-MS devices jump to `DeviceID="11"`+; document
order, not the `DeviceID` value, is what's consistent.

`<ModelNumber>` is the real vendor-assigned instrument identifier and is
what `openaraw` now maps to a PSI-MS CV term (see
`instrument_cv_for_model` in `crates/openaraw/src/reader.rs`), replacing
the previous guess that only distinguished "record stride >= 220" (Q-TOF)
from "record stride < 220" (QQQ) and tagged either case with the same
generic `MS:1000461` CV term.

Not every model number in the corpus has a dedicated PSI-MS CV term as
of `psi-ms.obo`'s current revision - `G6540A`/`G6540B` only have the
unsuffixed `6540 Q-TOF LC/MS` (`MS:1002789`) at the family level, and
`G6550B` has no entry at all (only `6550`/`6550A`). For these, the reader
falls back to the generic `MS:1000490` "Agilent instrument model" term
rather than mapping to a same-family term that would misidentify the
exact hardware revision.

## Known Gap: Missing Sidecars

`PXD031771/655_2.d` is missing both `Devices.xml` and `Contents.xml`
despite having a normal `AcqData/MSScan.bin` (its two siblings,
`655_1.d` and `655_3.d`, have both files - this looks like an incomplete
per-file export rather than a distinct bundle shape). The reader treats
both files as optional: `Devices.xml` absence falls back to the legacy
stride-based instrument guess, and `Contents.xml` absence leaves
`start_timestamp` as `None`, exactly as before this fix for bundles
lacking these files.
