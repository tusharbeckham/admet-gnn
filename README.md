# admet-gnn

**ADMETriage — A Computational Drug Discovery Engine for ADMET Screening**

Predicts early-stage ADMET behaviour (absorption, distribution, metabolism,
excretion, toxicity) for small molecules from a SMILES string. A graph neural
network trained on Therapeutics Data Commons benchmarks is exported to ONNX and
served through a Rust inference API, with a SvelteKit workspace on top.

**→ If you are here to work on this, read [`START-HERE.md`](START-HERE.md).** It is the
week-by-week plan with the exact commands.

**Status — 2026-08-31, at the scaffold tag.** Planning and design are complete: SRS,
design document, test plan, traceability matrix and seven ADRs are written. The Rust
toolchain is now installed and **the workspace compiles**: all five crates build clean,
`cargo clippy --all-features -- -D warnings` is silent, and `cargo nextest run --workspace`
reports **119 passed, 17 skipped** — the 17 being `#[ignore]`d placeholders that name
Increment 1–5 work rather than pretending to cover it. `cargo doc` is a gate too, so a
comment cannot link to a method that does not exist. CI is green on all four jobs.

The gate that matters: `tests/onnx_parity.rs` **passes**, agreeing with Python's
`onnxruntime` within tolerance across batch sizes 1 and 3. That closes the Python↔Rust
loop ADR-01 rests on, and it had never once executed before this.

Compiling a scaffold that had never been built cost **eleven defects**, DEF-01…DEF-11 in
[`docs/03-test-plan.md`](docs/03-test-plan.md) §10.2, three of them S1 — a wrong number
that looks right. The committed ONNX fixture could not be loaded at all; `MolGraph::default()`
violated the CSR invariant it validates itself against; and one fatal endpoint scored 0.119
instead of ~0, meaning the triage score did not actually disqualify a likely hERG blocker.
Evidence in [`docs/evidence/increment-0/`](docs/evidence/increment-0/).

Still absent, deliberately: the TDC downloader is written but nothing has been downloaded,
so `data/` still holds the superseded MoleculeNet prototype. No featuriser body, no GIN, no
training run, no migrations, no front end.

So nothing here is trained or benchmarked. Every performance figure in `docs/` is a
*target*, not a measurement, and is labelled as such. A skipped test is not a passing test,
and 119 passing tests over stubs say the scaffold is sound — not that the system works.


---

## Architecture in one line

**Python trains. Rust serves. A single ONNX artefact joins them.**

```
SMILES
  │
  ├─► admet-core (Rust)   parse, canonicalise, 33-dim atom features,
  │                        dense adjacency, Morgan fingerprint
  │
  ├─► admet-infer (Rust)  ONNX Runtime via `ort` → 12 endpoint heads
  │                        + applicability-domain check
  │
  ├─► admet-db (Rust)     PostgreSQL 16, results cached by InChIKey
  │
  └─► admet-api (Rust)    Axum 0.8, REST + batch CSV streaming
         │
         └─► SvelteKit 2 workspace  ·  Tauri v2 desktop build
```

The Python side (`training/`) exists only to produce `model.onnx`. It is never
on the request path.

## Why the model is a *dense* GNN and not PyTorch Geometric

This is the single most important design constraint in the project, so it is
stated first.

PyTorch Geometric represents a batch of molecules as one big sparse graph and
aggregates neighbours with scatter operations (`torch_scatter`). Those
operations **have no ONNX equivalent**, so a PyG model cannot be exported to a
portable artefact — which means it cannot be served from Rust.

So the model uses **padded dense adjacency** instead:

| Tensor | Shape | Meaning |
|---|---|---|
| `x` | `[B, 128, 33]` | atom features, zero-padded |
| `adj` | `[B, 128, 128]` | symmetric-normalised adjacency `D^-½ A D^-½` |
| `mask` | `[B, 128]` | 1 for real atoms, 0 for padding |

Message passing becomes `torch.bmm(adj, x)` — a batched matrix multiply, which
ONNX exports cleanly at opset 17. The cost is arithmetic on padding
(≈2.1M multiply-adds dense vs ≈17k sparse for a typical 30-atom molecule).
That waste is irrelevant on CPU at this scale and buys the entire Rust serving
path. **Molecules above 128 heavy atoms are rejected, not truncated.**

## Endpoints (Therapeutics Data Commons, ADMET group)

| # | Endpoint | Task | n |
|---|---|---|---|
| E01 | `Caco2_Wang` | regression | 906 |
| E02 | `HIA_Hou` | binary | 578 |
| E03 | `Pgp_Broccatelli` | binary | 1,212 |
| E04 | `Bioavailability_Ma` | binary | 640 |
| E05 | `BBB_Martins` | binary | 1,975 |
| E06 | `PPBR_AZ` | regression | 1,797 |
| E07 | `VDss_Lombardo` | regression | 1,130 |
| E08 | `CYP3A4_Veith` | binary | 12,328 |
| E09 | `CYP2D6_Veith` | binary | 13,130 |
| E10 | `Half_Life_Obach` | regression | 667 |
| E11 | `Clearance_Hepatocyte_AZ` | regression | 1,213 |
| E12 | `hERG` | binary | 648 |

Splitting is **Bemis–Murcko scaffold split**, never random. Random splits leak
near-duplicate molecules between train and test and inflate reported scores by
roughly 10–20 percentage points. This is the most common error in molecular ML
benchmarking and any reported number that used a random split is meaningless.

## Increments

The project follows an **Incremental process model with a V-shaped
verification spine** — each increment ships working software, its slice of
documentation, and the tests that prove its requirements.

| Tag | Increment | Delivers |
|---|---|---|
| `v0.1.0-model` | 1 — Model core | trained dense GIN exported to `model.onnx`, parity fixture |
| `v0.2.0-api` | 2 — Inference service | `admet-core`/`admet-infer`/`admet-db`/`admet-api` |
| `v0.3.0-web` | 3 — Web workspace | SvelteKit UI, single-molecule report |
| `v0.4.0-batch` | 4 — Batch + explainability | CSV screening, per-atom attribution, triage ranking |
| `v0.5.0-desktop` | 5 — Packaging | Tauri desktop build, Docker image, CI/CD |

## Quickstart

Development is **native Windows with Git Bash** — not WSL2. That is a deliberate,
documented deviation from the build manual's advice
([ADR-07](docs/adr/0007-native-windows-and-training-layout.md)), and the Windows-specific
failure modes it costs are written up in
[`docs/reference/troubleshooting.md`](docs/reference/troubleshooting.md).

```bash
bash scripts/verify-env.sh
```

Red rows are the script working. Install what it names from
[`docs/00-machine-setup.md`](docs/00-machine-setup.md), then:

```bash
just setup
```

```bash
just spike
```

`just spike` is the week-one gate: it builds an untrained dense GIN, exports it to ONNX,
reloads it in `onnxruntime`, and asserts the outputs match across batch sizes 1 / 7 / 64.
It needs no data and takes seconds. If it fails, the architecture's one fatal assumption
is wrong and the fallback is decided immediately rather than in week 8.

```bash
just build && just check
```

Data acquisition uses a **second** virtual environment, because PyTDC pins
`rdkit<2024.3.1` while this project runs 2026.3.5 and mixing them silently downgrades the
chemistry library the parity fixture was built against:

```bash
just setup-tdc && just data-download && just data-profile
```

Every recipe with its raw command is in
[`docs/reference/commands.md`](docs/reference/commands.md); `just --list` prints them.

## Documentation map

| | |
|---|---|
| [`START-HERE.md`](START-HERE.md) | the fifteen weeks, in order, with commands |
| [`requirements.md`](requirements.md) | the authoritative FR/TR/NFR/UC register |
| [`implementation.md`](implementation.md) | process model, increments, schedule, order of attack |
| [`docs/01-srs.md`](docs/01-srs.md) · [`02-design.md`](docs/02-design.md) · [`03-test-plan.md`](docs/03-test-plan.md) | the graded documents |
| [`docs/04-traceability.md`](docs/04-traceability.md) | requirement ↔ code ↔ test, machine-checkable |
| [`docs/adr/`](docs/adr/README.md) | seven decisions and why |
| [`docs/reference/`](docs/reference/commands.md) | commands, glossary, troubleshooting, manual index |
| [`docs/journal/`](docs/journal/README.md) | weekly entries — the evaluation chapter's raw material |


## Repository layout

As it actually is today. `⚠` marks a directory holding only a README that states what
lands there and in which increment.

```
Phore/
├── START-HERE.md            the week-by-week build guide
├── requirements.md          FR/TR/NFR/UC register — authoritative
├── implementation.md        process model, increments, 15-week schedule
├── research.md  method.md   background and methodology
├── MIGRATION.md             why MoleculeNet was retired for TDC
├── justfile                 every routine command
├── Cargo.toml               workspace: crates/*
├── rust-toolchain.toml  .nvmrc  .env.example
├── training/                Python — trains, then leaves the request path
│   ├── data/                download_tdc.py  profile.py  clean.py  scaffold_split.py
│   ├── scripts/             spike_onnx_export.py  dump_parity_fixture.py
│   └── legacy_moleculenet/  superseded prototype, kept as migration evidence
├── crates/
│   ├── admet-core/          graph · smiles/{lexer,parser,ring} · canonical · features
│   │                        fingerprint · scaffold · triage · benches/core.rs
│   ├── admet-infer/         ort session, batching, applicability domain  (+ onnx_parity test)
│   ├── admet-db/            model.rs (ER rows) · repository/{molecule,prediction,batch}
│   ├── admet-api/           routes/{predict,health} · error (RFC 9457) · state · config · tracing
│   └── admet-cli/           predict · import · bench · schema
├── config/default.toml      layered configuration
├── migrations/ ⚠            sqlx migrations — Increment 2
├── models/ ⚠                model.onnx + feature_schema.json — Increment 1
├── web/ ⚠                   SvelteKit 2 workspace — Increment 3
├── desktop/ ⚠               Tauri v2 shell — Increment 5
├── fixtures/                golden parity vectors (binary — see .gitattributes)
├── results/                 measured numbers only, written by `just results`
├── scripts/                 verify-env.sh  verify-env.ps1
├── docs/
│   ├── 00-machine-setup.md  01-srs.md  02-design.md  03-test-plan.md
│   ├── 04-traceability.md   05-git-conventions.md  06-data-sources.md
│   ├── adr/                 ADR-01 … ADR-07
│   ├── diagrams/            16-diagram register, tools, consistency rules
│   ├── evidence/            observations a script cannot regenerate
│   ├── journal/             week-01 … week-15 + template
│   └── reference/           commands · glossary · troubleshooting · build-manual-index
├── .githooks/pre-commit     versioned; installed by `just hooks`
└── .github/workflows/ci.yml four jobs; the web job is off until Increment 3
```

The Python side exists only to produce `model.onnx` and `feature_schema.json`. It is never
on the request path (TR-02) — which is the whole reason `admet-core` reimplements the
33-dim featuriser in Rust, and the reason the parity fixture exists to keep the two
honest (TR-03, risk R3).

## Honesty note

The design documents in `docs/` contain performance targets (p95 latency
under 300 ms, mean AUROC ≥ 0.80, 10k-molecule batch under 90 s). These are
goals set during design. They are **not** results. No figure moves out of
`docs/` and into `results/` until it has been produced by a script in this
repository on a scaffold-held-out test split, and `results/` records what
actually happened — including the endpoints that miss their target.

## Licence

See `LICENSE`. Note that the repository is currently public; an
all-rights-reserved licence restricts *reuse*, not *visibility*.
