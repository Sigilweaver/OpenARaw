# OpenARaw Validation Corpus

Current size: 129.5 GB logical (~85 GB on disk after ZFS compression)
across 29 PRIDE projects, 338 `.d` directories. 334 of the 338 are
`AcqData` shape (the modern MassHunter LC-MS layout this reader targets);
the remaining 4 are structurally malformed source uploads, not a
distinct format variant - see
[docs/format/05-known-limitations.md](docs/format/05-known-limitations.md).

The corpus covers both Q-TOF (profile and centroid/peak acquisition) and
QQQ (MRM) instrument families. `docs/format/01-msscan.md` has the
per-file instrument identification (via `Devices.xml`/`Contents.xml`) for
the specific files used to validate the binary format itself; PRIDE's own
per-file instrument metadata is too inconsistent (generic placeholder
tags, occasional literal `"QQQ"`, frequently blank) to use for a reliable
corpus-wide instrument breakdown, so this document does not attempt one.

**No legacy `DATA.MS` / ChemStation GC-MS directories exist in the
corpus.** PRIDE is proteomics-focused and skews heavily toward modern
LC-MS submissions, so that folder shape was never found there; if a
ChemStation/GC-MS example is needed later it will likely have to come
from a non-PRIDE source.

## Source: PRIDE Archive

All files come from the EBI PRIDE Archive (https://www.ebi.ac.uk/pride/),
a public proteomics repository:

    Perez-Riverol Y et al. "The PRIDE database and related tools and resources in 2019:
    improving support for quantification data." Nucleic Acids Res. 2019;47(D1):D442-D450.
    doi:10.1093/nar/gky1106

PRIDE datasets are published under CC-BY or equivalent open licences.

## Fetch tooling

`re/src/analysis/pride.py` (gitignored, local-only research tooling) is a
small CLI over the PRIDE REST API with four commands:

    python -m analysis.pride search <query>     # find Agilent projects
    python -m analysis.pride files <accession>   # list a project's .d files
    python -m analysis.pride fetch <accession>   # download + extract .d.zip files
    python -m analysis.pride catalog             # rebuild Data/ARaw/index.csv

Unlike OpenTFRaw's Thermo corpus, Agilent submitters do not consistently
name their archives `<name>.d.zip` - some projects ship a single
descriptively-named `.zip` with no `.d` hint at all. The `.d.zip`/`.d`
filename heuristic in `pride.py` therefore under-counts real datasets in
some projects; building the actual source list required falling back to
manual review per project rather than a fully automatic filter.

## Provenance Record

`Data/ARaw/index.csv` records which PRIDE project each local `.d`
directory came from, plus its size and detected folder shape. To trace
any file back to its source, use the PRIDE accession:

    https://www.ebi.ac.uk/pride/archive/projects/<PXD_ACCESSION>

## Limitations

- PRIDE's per-file instrument metadata is unreliable (see above);
  instrument identification for format-documentation purposes was done
  per-file from `Devices.xml`/`Contents.xml`, not corpus-wide.
- 4 of the 338 cataloged directories came back with an `unknown` folder
  shape from automated detection; combined with 2 further directories
  that turned out to be malformed only on deeper inspection, 6 total
  directories are excluded from conformance testing as genuinely
  malformed source data (nested duplicate folders, macOS resource-fork
  artifacts) - see
  [docs/format/05-known-limitations.md](docs/format/05-known-limitations.md)
  for the exact list.
