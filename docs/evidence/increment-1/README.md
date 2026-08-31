# Increment 1, step 1 — data acquisition and profile

Captured **2026-08-31**. This is the first time TDC data has been downloaded for
this project: before now the downloader existed but `data/` still held the
superseded MoleculeNet prototype.

| File | What it shows | Command |
|---|---|---|
| `tdc_manifest.json` | All twelve endpoints fetched, with per-endpoint row and unique-SMILES counts and the PyTDC version. | `.venv-tdc/Scripts/python.exe training/data/download_tdc.py` |
| `data-profile.txt` | The exploratory profile: rows, unparseable count, heavy-atom mean/max, count over the 128 cap, duplicates, and target distribution per endpoint. | `just data-profile` |
| `data_profile.json` | The same, machine-readable, for the report tables. | as above |

## What the data actually says

**37,289 labelled rows across 12 endpoints, and zero unparseable SMILES.** The
smallest endpoint (`hia`, 578) has two orders of magnitude fewer rows than the
largest (`cyp2d6`, 13,130) — which is the argument for a multi-task shared trunk
stated as a measurement rather than an assumption.

Findings that change how Increment 1 must be built:

| Finding | Consequence |
|---|---|
| `hia` is 86.5% positive, `cyp2d6` 19.1% | Class weighting is required, not optional: `pos_weight` 0.156 and 4.223 respectively. |
| `vdss` skew **+27.1**, `half_life` **+10.3**, `ppbr` **−2.0**, `clearance` **+1.2** | Four regression targets are long-tailed. Huber loss, not MSE — MSE on a skew-27 target optimises for the outliers. |
| **8 molecules** exceed the 128-atom cap (1 `bbb`, 6 `vdss`, 1 `half_life`) | The cap costs 0.02% of the data. ADR-03's fixed atom axis is cheap, and confirmed so with a number rather than a hope. |
| `ppbr` has **993 duplicate SMILES in 2,790 rows** (36%), `clearance` 193, `bbb` 55 | Deduplication is a real pipeline step, and `clean.py`'s label-conflict report matters more than expected. |
| Max heavy atoms reaches **155** (`vdss`) | Above the cap, so the rejection path is exercised by real data, not just tests. |

## The row-count scare, and why it was not one

The downloader initially flagged `DRIFT` on five endpoints — `ppbr` at 2,790 rows
against a documented 1,797 is the kind of gap that means a dataset was revised and
every published comparison is now apples to oranges.

It was nothing of the sort. The published TDC figures are **unique-molecule**
counts; the benchmark-group CSVs carry duplicate SMILES.
`training/scripts/check_row_count_drift.py` verifies this for all twelve endpoints:
`rows − duplicates` reproduces the published figure exactly, or the raw count
already matched. **12 explained, 0 unexplained.** Logged as DEF-13, and the check
now compares unique SMILES so the warning means something when it fires.

## Honesty notes

- No chemistry was performed in `.venv-tdc`. It wrote raw CSV and nothing else, per
  the hard rule in `requirements-data.txt` — so the two-RDKit hazard (risk R3) was
  never in play. `rdkit` is not even installed there.
- Profiling ran in the main `.venv` against the single pinned RDKit 2026.03.5.
- These are **descriptive** statistics. No split has been made, no molecule
  featurised, no model trained. Nothing here is a performance result.
- The documented row counts in `docs/06-data-sources.md` are left as published:
  they are correct as unique-molecule counts, and rewriting them to raw row counts
  would break comparison with the TDC paper.
