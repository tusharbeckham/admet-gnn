# ADR-06: Use the TDC ADMET benchmark group, twelve endpoints, frozen

- **Status**: Accepted
- **Date**: 2026-08-17
- **Deciders**: tusharbeckham
- **Related**: FR-08, TR-12, NFR-03, G1, G8, R2, R3, R5
- **Supersedes**: the MoleculeNet prototype (retained as
  [`training/legacy_moleculenet/`](../../training/legacy_moleculenet/))

## Context

The first version of this project used MoleculeNet — ESOL, Tox21 and BBBP — and there
is working code for it in the repository history. That choice needs revisiting
because the project's stated question changed from "can a GNN predict molecular
properties" to "can a GNN triage compounds on ADMET risk", and the second question
has different data requirements.

MoleculeNet (Wu et al., 2018) is older, better known, and more widely cited.
Therapeutics Data Commons (Huang et al., 2021) is newer, curated for therapeutic ML
specifically, and provides an ADMET benchmark group with published leaderboards.

Three things decide it.

**The endpoints have to actually be ADMET.** ESOL is aqueous solubility — useful,
and not pharmacokinetics. Tox21 is a panel of *in-vitro* nuclear-receptor and
stress-response assays — useful, and not pharmacokinetics either. A "clearance"
prediction and a "does this activate the androgen receptor" prediction are not the
same kind of claim, and a triage tool built on the second cannot answer the first.

**Curation.** TDC applies unit harmonisation and deduplication that MoleculeNet's raw
CSVs do not, and documents provenance per dataset. MoleculeNet's Tox21 CSV has
missing-value conventions that differ between columns.

**Access.** MoleculeNet is distributed mainly through DeepChem, whose PyPI releases
lag its source by years and frequently fail to resolve on current Python versions.
`pip install PyTDC` works.

## Decision

Data comes from the **TDC ADMET Benchmark Group**. Twelve endpoints, chosen to cover
all five ADMET letters, and **frozen** for the duration of the project:

| # | Endpoint | Letter | Task | n |
|---|---|---|---|---|
| E01 | `Caco2_Wang` | A | regression | 906 |
| E02 | `HIA_Hou` | A | binary | 578 |
| E03 | `Pgp_Broccatelli` | A/D | binary | 1,212 |
| E04 | `Bioavailability_Ma` | A | binary | 640 |
| E05 | `BBB_Martins` | D | binary | 1,975 |
| E06 | `PPBR_AZ` | D | regression | 1,797 |
| E07 | `VDss_Lombardo` | D | regression | 1,130 |
| E08 | `CYP3A4_Veith` | M | binary | 12,328 |
| E09 | `CYP2D6_Veith` | M | binary | 13,130 |
| E10 | `Half_Life_Obach` | E | regression | 667 |
| E11 | `Clearance_Hepatocyte_AZ` | E | regression | 1,213 |
| E12 | `hERG` | T | binary | 648 |

TDC offers twenty-two. Twelve is the scope that fits fifteen weeks: broad enough to
be a platform, small enough to finish. **"Frozen" is the operative word** — the
endpoint list is baked into the model's output dimension (`N_ENDPOINTS = 12`), the
database's `endpoints` table, the ONNX graph's output shape, and the UI's radar
chart. Adding a thirteenth in week eleven is a retrain plus a migration plus a
schema-version bump, not a config change.

**TDC runs in a separate throwaway environment (`.venv-tdc`) and performs zero
chemistry.** It writes raw CSV and nothing else.

## Consequences

### Positive

- **The endpoints answer the project's actual question.** Absorption, distribution,
  metabolism, excretion and toxicity, with at least one endpoint each.
- **External anchors for every number.** TDC publishes leaderboards per endpoint, so
  each reported metric sits next to a published result. That converts "0.76 AUC" from
  an unverifiable claim into a comparison — and being 0.03 below a state-of-the-art
  ensemble is a *good* result to report honestly.
- **Prescribed splits and seeds.** TDC specifies five split seeds, which removes an
  arbitrary choice and makes mean ± sd the natural reporting format.
- **Multi-task learning is well-posed.** ~37,000 rows across twelve endpoints, with
  seven endpoints under 1,300 rows each. A shared trunk lets the small endpoints
  borrow a representation learned from all of them, which is the main methodological
  reason this dataset shape works at all.
- **`BBB_Martins` covers the same question as MoleculeNet's BBBP with better
  curation**, so the prototype work is not wasted — it transfers.

### Negative

- **PyTDC's dependency tree is hostile.** It pins `rdkit>=2023.9.5,<2024.3.1` while
  this project runs 2026.3.5, and it pulls in the HuggingFace training stack
  (`transformers`, `accelerate`, `datasets`, `evaluate`, `huggingface-hub`) plus two
  single-cell genomics packages (`cellxgene-census`, `tiledbsoma`). None of that
  belongs in an environment whose only job is to produce one ONNX file.
  (TDC issue #374.)

  Mitigation, and it is a **hard rule** rather than a preference: TDC lives in a
  disposable `.venv-tdc`, and anything run in it **writes raw CSV and performs no
  chemistry**. No canonicalisation, no salt stripping, no InChIKey, no scaffold
  assignment, no featurisation.

  The reason is risk R3 in its most insidious form. Features computed by RDKit
  2023.09 and features computed by RDKit 2026.03 can differ in aromaticity
  perception, ring finding and stereo handling. If half the pipeline runs on each,
  the training set and the serving path disagree in ways no test catches and no
  metric explains. One RDKit, one feature contract.

- **Datasets get revised between PyTDC releases.** A benchmark number is meaningless
  without the version that produced it. `download_tdc.py` writes `pytdc_version`
  into `training/data/raw/tdc/_manifest.json` and flags row-count **drift** against
  the published sizes above; a `DRIFT` warning goes in the weekly log rather than
  being ignored.
- **Two Python environments to maintain.** Two requirements files, two venvs, and a
  documented order of operations. It is genuinely more setup than one environment,
  and it is the price of not corrupting the feature contract.
- **Smaller community than MoleculeNet.** Fewer Stack Overflow answers, fewer
  tutorials, and the API has changed shape across releases.
- **Twelve is a commitment, not a starting point.** Freezing means the output
  dimension is fixed everywhere. That is the correct trade — a movable endpoint list
  means the ONNX artefact, the database and the UI can silently disagree about which
  index is `hERG`, which is a wrong prediction with a plausible number attached.

### Neutral

- The MoleculeNet code is **retired, not deleted**. It lives in
  `training/legacy_moleculenet/` with a README explaining what it was, because it is
  the evidence behind this ADR and behind `MIGRATION.md`. Deleting it would leave
  this document asserting a comparison nobody can check.
- TDC's `get_split(method='scaffold')` is a cross-check; the authoritative split is
  this repo's own — see [ADR-05](0005-scaffold-split-not-random.md).

## Alternatives considered

### Stay with MoleculeNet

Least work — the download and cleaning code already existed and ran. Rejected on
the first point above: ESOL and Tox21 are not ADMET, so the platform would be
answering a different question from the one in its own title. DeepChem's packaging
was a secondary but real factor.

### Both MoleculeNet and TDC

More data, more endpoints, more coverage. Rejected because it doubles the ingestion,
cleaning and evaluation surface, and the two suites overlap on the endpoints that
matter (BBBP/`BBB_Martins`) while disagreeing on units and missing-value conventions
where they do not. The marginal endpoint is not worth the second pipeline.

### ChEMBL directly

The largest and most authoritative source. Rejected on scope: raw ChEMBL requires
assay-level curation — deciding which assay measures the endpoint, harmonising units
across labs, handling censored values — which is a project in itself and is
*precisely* the work TDC has already done and documented.

### All twenty-two TDC endpoints

Tempting, since the pipeline is the same. Rejected on time: twenty-two endpoints
means twenty-two sets of hyperparameters, twenty-two calibrations, twenty-two
leaderboard comparisons and twenty-two rows in every results table. Twelve already
covers all five letters, and the marginal ten endpoints add breadth to a table
rather than capability to the tool.

### Vendor an offline copy of the twelve CSVs and drop PyTDC entirely

Removes the dependency conflict at a stroke. Rejected because it loses the version
provenance — the whole reason `pytdc_version` is recorded is that datasets get
revised, and a vendored CSV has no revision. Also, committing ~37,000 rows of CSV
is exactly what `.gitignore` forbids. The `.venv-tdc` split achieves the same
isolation without discarding traceability.

## References

- Huang et al. (2021), *Therapeutics Data Commons*, NeurIPS Datasets & Benchmarks
- TDC ADMET group: <https://tdcommons.ai/benchmark/admet_group/overview/>
- TDC issue #374 — the rdkit pin
- `research.md` §5 and §6
- [`docs/06-data-sources.md`](../06-data-sources.md) — endpoints, licences, resource
  directory
- [`requirements-data.txt`](../../requirements-data.txt) — the hard rule, stated where
  it will be read
