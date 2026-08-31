# Superseded — MoleculeNet prototype

**Nothing in this directory is on the current build path.** It is kept
deliberately, and this file explains why.

## What this was

The first version of the project used three **MoleculeNet** datasets:

| Dataset | Task | What it measures |
|---|---|---|
| ESOL (Delaney) | regression | aqueous solubility |
| BBBP | binary | blood–brain barrier penetration |
| Tox21 | 12 binary tasks | nuclear-receptor and stress-response assays |

`download_moleculenet.py` fetched them from the `deepchemdata` S3 bucket,
`clean_moleculenet.py` cleaned them, and `run_phase1.py` ran the whole
download → clean → scaffold-split → save pipeline. That pipeline worked; the
processed CSVs it produced are still in `data/processed/`.

## Why it was replaced

The endpoints were the problem, not the code. Recorded in full as
[ADR-06](../../docs/adr/ADR-06-tdc-over-moleculenet.md); in short:

1. **They are not ADMET.** ESOL is solubility — a physicochemical property that
   *influences* absorption but is not itself an ADMET endpoint. Tox21 is a panel
   of *in-vitro* assays, not pharmacokinetics. The project's thesis is early
   ADMET triage, and TDC's ADMET group is built for exactly that question.
2. **Curation.** TDC applies unit harmonisation and deduplication that
   MoleculeNet's raw CSVs do not, and documents provenance per dataset.
3. **An external anchor.** TDC publishes leaderboards, so every number this
   project reports can be positioned against published work. That is worth real
   marks in a viva and MoleculeNet gives you nothing comparable.
4. **Access.** MoleculeNet is distributed mainly through DeepChem, whose PyPI
   releases lag its source by years and frequently fail to resolve on current
   Python. `pip install PyTDC` works.

The overlap is not wasted: TDC's `BBB_Martins` asks the same question as BBBP
with better curation.

## Why keep it rather than delete it

Three reasons, in order of how much they matter:

1. **It is the evidence for a design decision.** "Why did the endpoints change
   halfway through?" is a reasonable viva question, and pointing at a superseded
   pipeline plus an ADR is a much stronger answer than describing one from
   memory. Deleting it would leave `MIGRATION.md` making a claim with nothing
   behind it.

2. **It demonstrates that the abstraction held.** These three cleaning functions
   are dataset-specific *wrappers*. The generic machinery they call —
   `canonicalize()` and `clean_dataset()` in `training/data/clean.py` — survived
   the migration completely unchanged, as did
   `training/data/scaffold_split.py`. Swapping the entire dataset cost three
   wrapper functions. That is the payoff for having isolated dataset knowledge
   in the first place, and it is worth a sentence in the design chapter.

3. **`data/processed/` still holds its output.** Those CSVs are a working
   reference for what a cleaned, scaffold-split dataset should look like, which
   is useful while writing the TDC equivalent.

## What replaced what

| Superseded | Current |
|---|---|
| `download_moleculenet.py` | [`training/data/download_tdc.py`](../data/download_tdc.py) |
| `clean_moleculenet.py` | `training/data/clean_tdc.py` *(Increment 1)* |
| `run_phase1.py` | `justfile` recipes: `data-download`, `data-profile`, `data-prepare` |
| — *(no equivalent existed)* | [`training/data/profile.py`](../data/profile.py) |

Reused unchanged, not superseded:

- [`training/data/clean.py`](../data/clean.py) — `canonicalize()`, `clean_dataset()`
- [`training/data/scaffold_split.py`](../data/scaffold_split.py) — `get_scaffold()`, `scaffold_split()`

## Do these still run?

`clean_moleculenet.py` does — it shims `sys.path` to reach the generic module and
reads from `data/raw/`, where the CSVs actually are.

`download_moleculenet.py` and `run_phase1.py` have **stale relative paths**: both
resolve `raw/` and `processed/` next to themselves, which was correct when they
lived at `data/` in the repo root and stopped being correct when they moved into
`training/`. They are not fixed, because fixing them would imply they should be
run. They are here to be read, not executed.

> If you ever do need the MoleculeNet data again, it is faster to re-download it
> than to repair these paths.
