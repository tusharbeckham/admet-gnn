# admet-gnn

**ADMETriage — A Computational Drug Discovery Engine for ADMET Screening**

Predicts early-stage ADMET behaviour (absorption, distribution, metabolism,
excretion, toxicity) for small molecules from a SMILES string. A graph neural
network trained on Therapeutics Data Commons benchmarks is exported to ONNX and
served through a Rust inference API, with a SvelteKit workspace on top.

**Status:** Increment 1 (model core) — in progress. Nothing in this repository
is trained or benchmarked yet. Every performance figure in `docs/` is a
*target*, not a measurement, and is labelled as such.

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

```bash
# --- training side (Python 3.12) ---
curl -LsSf https://astral.sh/uv/install.sh | sh
cd training
uv venv && source .venv/bin/activate
uv pip install -r ../requirements.txt

# prove the ONNX round-trip works before writing anything else
python scripts/spike_onnx_export.py

# --- serving side (Rust 1.83+) ---
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo run -p admet-cli -- predict "CC(=O)Oc1ccccc1C(=O)O"
```

Everything routine is wrapped in `just`: `just train`, `just export`,
`just serve`, `just web`, `just test`, `just check`.

## Repository layout

```
admet-gnn/
├── training/            Python — data, featurisation, training, ONNX export
│   ├── data/            TDC download, cleaning, scaffold split
│   ├── features/        33-dim atom features, dense adjacency, descriptors
│   ├── models/          dense GIN, MLP fingerprint baseline
│   ├── scripts/         spike_onnx_export.py, train.py, export.py
│   └── results/         metrics, model cards, calibration
├── crates/
│   ├── admet-core/      SMILES parser, graph, features, fingerprint, triage
│   ├── admet-infer/     ONNX Runtime session, batching, applicability domain
│   ├── admet-db/        sqlx queries, migrations
│   ├── admet-api/       Axum handlers, OpenAPI
│   └── admet-cli/       local prediction, bulk import
├── web/                 SvelteKit 2 workspace
├── desktop/             Tauri v2 shell
├── docs/                SRS, DFDs, ER model, UML, test plan, ADRs
├── fixtures/            golden parity vectors (200 molecules)
└── justfile
```

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
