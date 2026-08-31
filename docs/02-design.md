# 02 — Software Design Description

| | |
|---|---|
| **System** | ADMETriage |
| **Document** | SDD. Architecture, components, data, and the security analysis. |
| **Status** | Structure baselined at the scaffold. Figures are specified but not yet drawn. |
| **Version** | 0.1 (scaffold) |
| **Traces to** | [`01-srs.md`](01-srs.md), [`requirements.md`](../requirements.md), [`adr/`](adr/) |

## How to read this document

Every design choice worth arguing about is in an **ADR**, not here. This document
says *what the system is*; the ADRs say *why it is not something else*. When a
section links an ADR, the link is the justification and is not duplicated.

Figures are **specified before they are drawn**. Each placeholder below carries
what the figure must show, what it must not show, and the mistake that most often
ruins that particular diagram. Drawing a diagram from a specification takes
twenty minutes; drawing it from memory takes an hour and produces something that
disagrees with the code. Tool assignments and export rules are in
[`diagrams/README.md`](diagrams/README.md).

---

## 1. Architectural overview

Two languages, one artefact between them, five Rust crates arranged so that
dependencies point inward.

```text
                    ┌──────────────────────────────────────┐
   HTTP / invoke →  │  admet-api  (axum)   admet-cli       │  adapters
                    │      │                    │          │
                    │      ├──── admet-db ──────┤          │  infrastructure
                    │      │      (sqlx)        │          │
                    │      └──── admet-infer ───┘          │  ONNX Runtime
                    │                │                     │
                    │           admet-core                 │  domain: no I/O
                    └──────────────────────────────────────┘
```

The single rule that gives this shape its value: **`admet-core` depends on
nothing that touches the outside world.** No `sqlx`, no `tokio`, no `reqwest`, no
filesystem. Chemistry is pure functions over owned data
([ADR-02](adr/0002-hexagonal-crate-split.md)).

Three consequences follow, and they are the reason for the constraint rather than
happy accidents:

1. **The domain suite runs in about a second**, so the pre-commit hook can run it
   on every commit. A gate that takes a minute is a gate that gets skipped.
2. **The desktop build is a wrapper, not a rewrite.** Tauri links `admet-core`
   and `admet-infer` directly and swaps `fetch` for `invoke`. A `PgPool` in the
   domain crate would make Increment 5 a port.
3. **A chemistry bug cannot hide behind a mock.** There is nothing to mock: the
   inputs are `&str` and `MolGraph`, the outputs are values.

### 1.1 Crate responsibilities

| Crate | Owns | Must never contain |
|---|---|---|
| `admet-core` | SMILES lexer/parser, `MolGraph`, canonicalisation, the 33-dim featuriser, fingerprints, Bemis–Murcko scaffold, triage scoring | Any I/O, any async, any dependency with a network or disk surface |
| `admet-infer` | ONNX Runtime session lifecycle, tensor packing, batch dispatch, the `ort` version pin | HTTP, SQL, business rules about *when* to predict |
| `admet-db` | Schema, migrations, repositories, `sqlx` types | Chemistry. A descriptor computed in SQL is a second implementation (R3) |
| `admet-api` | Routing, extraction, RFC 9457 mapping, middleware, state | Chemistry, or SQL written inline in a handler |
| `admet-cli` | The same core without a server: `predict`, `import`, `bench`, `schema` | Anything the API needs — it is a consumer, not a shared layer |

`admet-api` is split into a library and a thin binary. The reason is mechanical
rather than aesthetic: in a binary crate `pub` does not exempt an unused item from
`dead_code`, so a scaffold with handlers not yet wired up cannot compile under
`-D warnings`. As a library it can.

---

## 2. Figures

Eight figures belong to this document. The other eight in the project's set of
sixteen belong to the SRS, the test plan and the report — see
[`diagrams/README.md`](diagrams/README.md).

### Fig 7.1 — System context (C4 level 1)

> **Status:** not drawn. `docs/diagrams/exports/fig-7.1-context.svg`

**Must show:** ADMETriage as one box. Two human actors, Chemist and Project lead.
Two external things it reads *at build time only* — the TDC datasets and RDKit.
Zero external systems on the request path.

**Must not show:** crates, HTTP, database. This is the level at which a
non-engineer can nod along.

**The mistake to avoid:** drawing the training data as a runtime dependency. It is
not, and that distinction is the whole of G6 — the arrow from TDC ends at
`model.onnx`, offline, before the system runs.

### Fig 7.2 — Containers (C4 level 2)

> **Status:** not drawn. `exports/fig-7.2-containers.svg`

**Must show:** browser (SvelteKit) → Caddy (TLS) → `admet-api` → PostgreSQL 16,
plus `admet-api` → ONNX Runtime in-process, plus the Tauri desktop container
holding the same core with SQLite. Label every arrow with protocol *and* the
requirement it serves (e.g. `HTTPS/JSON — FR-01`).

**Must not show:** functions or modules.

**The mistake to avoid:** drawing ONNX Runtime as a separate service. It is a
library inside the API process — that is TR-02, and a diagram implying an
inference microservice contradicts the latency argument in
[ADR-01](adr/0001-rust-serving-onnx-boundary.md).

### Fig 7.3 — Components of `admet-api` (C4 level 3)

> **Status:** not drawn. `exports/fig-7.3-components.svg`

**Must show:** middleware stack in order (CORS → body limit → timeout → trace),
router, the handler set, `AppState`, and the boundary where a handler stops and a
repository or the engine begins.

**The mistake to avoid:** drawing the middleware in the order it appears in the
source. `axum` layers apply **inside-out**, so the visual order must be the
*execution* order, or the diagram teaches the wrong thing. The body limit sits
outside the timeout deliberately: otherwise a 500 MB upload gets thirty seconds to
consume memory before anything checks its size.

### Fig 7.4 — Domain model of `admet-core`

> **Status:** not drawn. `exports/fig-7.4-domain.svg`

**Must show:** `MolGraph` with its structure-of-arrays layout and CSR adjacency,
`Atom`, `Bond`, `FeatureVector`, `Descriptors`, `Prediction`, `DomainFlag`, and the
error enum. Show ownership and cardinality.

**The mistake to avoid:** drawing it as UML classes with methods and getters. This
is Rust: the interesting content is *which types own which buffers* and where the
33 feature columns come from. A getter list conveys nothing.

### Fig 7.5 — Sequence: single prediction (UC-01)

> **Status:** not drawn. `exports/fig-7.5-seq-predict.svg`

**Must show:** the happy path with the cache **hit and miss branches both drawn**,
from `POST /predict` through validation (FR-02/03/04), canonicalisation and
InChIKey (FR-05), cache probe on `(inchikey, model_version)` (TR-05),
featurisation, `Engine::run`, domain check (FR-10/11), response assembly. Mark
where the 300 ms budget (NFR-01) is spent.

**The mistake to avoid:** omitting the cache-miss path, or drawing validation as
one box. The byte-offset error is a *requirement* (FR-02); if the diagram cannot
show where the offset comes from, the design has not been thought through.

### Fig 7.6 — Sequence: batch screening with backpressure (UC-02)

> **Status:** not drawn. `exports/fig-7.6-seq-batch.svg`

**Must show:** CSV upload, the **bounded** channel, the worker pool, the
micro-batch coalescer (up to 64), checkpoints every 250 rows, SSE progress
events, and — explicitly — what happens when the channel is full.

**The mistake to avoid:** drawing an unbounded queue. Backpressure *is* the
design (TR-08, R7); a diagram without a full-channel case has drawn the OOM
rather than the mitigation.

### Fig 7.7 — Entity relationships

> **Status:** not drawn. `exports/fig-7.7-erd.svg`

**Must show:** `molecules` (InChIKey `CHAR(27)` primary key), `model_versions`,
`predictions` (composite unique on molecule + model version, mirroring TR-05),
`batches`, `batch_rows`, `users`, `projects`, `project_molecules`. Show every
foreign key and every index that exists for a named query.

**The mistake to avoid:** a surrogate `id` on `molecules` alongside the InChIKey.
Two identities for one thing is how duplicates appear later. The key is the key
([ADR-04](adr/0004-inchikey-identity-and-cache-key.md)).

### Fig 7.8 — Deployment

> **Status:** not drawn. `exports/fig-7.8-deployment.svg`

**Must show:** both topologies side by side. Server: Docker Compose with Caddy,
`admet-api`, PostgreSQL, named volumes, and the `model.onnx` mount. Desktop:
a single Windows process with WebView2, the embedded model as a Tauri resource,
and SQLite in the user data directory. Mark the trust boundary on each.

**The mistake to avoid:** showing the desktop build downloading the model. It is
**embedded** as a resource — that is what makes FR-24 true with the network
interface disabled.

---

## 3. Key design decisions, in one line each

Each links the ADR that argues it. If a line here and an ADR disagree, the ADR is
correct — it is the record, this is the index.

| Area | Decision | ADR |
|---|---|---|
| Language seam | Python trains, Rust serves, `model.onnx` is the only artefact | [ADR-01](adr/0001-rust-serving-onnx-boundary.md) |
| Crate layout | Five crates, dependencies inward, `admet-core` I/O-free | [ADR-02](adr/0002-hexagonal-crate-split.md) |
| Graph representation | Padded dense adjacency and `torch.bmm`, not PyG scatter; 128-atom cap; reject, never truncate | [ADR-03](adr/0003-dense-adjacency-over-sparse-scatter.md) |
| Identity | InChIKey; cache keyed `(inchikey, model_version)` | [ADR-04](adr/0004-inchikey-identity-and-cache-key.md) |
| Evaluation | Bemis–Murcko scaffold split; both strategies reported; 5 seeds | [ADR-05](adr/0005-scaffold-split-not-random.md) |
| Data | TDC ADMET group, twelve endpoints, frozen | [ADR-06](adr/0006-tdc-over-moleculenet.md) |
| Environment | Native Windows over WSL2; `training/` over `ml/src/admet_ml/` | [ADR-07](adr/0007-native-windows-and-training-layout.md) |

## 4. Data design

### 4.1 The feature contract

33 columns per atom, in blocks, with the widths summing to exactly 33. The
authoritative definition is `admet_core::features`, and
`models/feature_schema.json` is **generated from it** —
`admet_core::features::schema_json()`, surfaced by `just schema`.

Python asserts against that file; it never writes it. The direction is the
mitigation for R3: reversed, a Python-side edit would silently redefine the
contract and the model would train on columns the server does not produce. The
failure mode being avoided is not a crash — it is a plausible wrong number, which
is why the tolerance is 1e-6 (TR-03) rather than "close enough".

Tensor shapes at the ONNX boundary, fixed except for the batch axis (TR-01):

| Name | Shape | Meaning |
|---|---|---|
| `x` | `[B, 128, 33]` f32 | Atom features, zero-padded above the atom count |
| `adj` | `[B, 128, 128]` f32 | Normalised adjacency with self-loops |
| `mask` | `[B, 128]` f32 | 1 for a real atom, 0 for padding |
| `y` | `[B, 12]` f32 | One output per endpoint, in the frozen endpoint order |

The endpoint **order** is part of the contract. Reordering it silently remaps
every prediction, so it is pinned by a test rather than by convention.

### 4.2 Relational schema

Detail in Fig 7.7 and `crates/admet-db/src/model.rs`. Three choices worth stating
in prose because they are the ones a reviewer will question:

- **`molecules.inchikey CHAR(27)` is the primary key**, not a surrogate integer.
  An InChIKey is fixed-width by specification, and a second identity for the same
  thing is how duplicate rows appear six months later.
- **`predictions` is unique on `(inchikey, model_version_id)`**, which is TR-05
  expressed as a constraint rather than as cache-invalidation code. A model
  upgrade cannot collide with old rows because it cannot address them.
- **`batch_rows.triage_score` is nullable on purpose** (FR-12). Withholding a
  score is a result, not a missing value, and it carries its reason in an adjacent
  column so the UI never has to guess why.

### 4.3 Caching

Two layers, and the outer one exists only because the inner one is expensive to
reach.

| Layer | Key | Scope | Invalidation |
|---|---|---|---|
| In-process, sharded LRU | `(inchikey, model_version)` | One process | Eviction by capacity; correctness by key construction |
| PostgreSQL `predictions` | `(inchikey, model_version_id)` | All processes | Never — a row for an old model version simply stops being addressed |

Sixteen shards, 50,000 entries total (~10 MB). Sharding keeps lock contention
low; the number is a starting point to be measured, not a tuned value, and
`just bench` is the argument that would change it.

## 5. Behavioural design

### 5.1 The error model

One error taxonomy, three surfaces (HTTP, CLI, Tauri). `admet-core` returns typed
errors with byte offsets; `admet-api` maps them to RFC 9457 problem details
(TR-09) with a `position` extension member.

| Domain error | HTTP | `type` suffix | Requirement |
|---|---|---|---|
| Input too long | 400 | `input-too-long` | FR-03 |
| Parse failure at offset | 400 | `invalid-smiles` | FR-02 |
| Too many heavy atoms | 422 | `molecule-too-large` | FR-04 |
| Out of applicability domain | 200 + flags | — (not an error) | FR-11, FR-12 |
| Model absent | 503 | `model-unavailable` | NFR-11 |
| Body over 20 MB | 413 | `payload-too-large` | TR-06 |
| Timeout | 504 | `timeout` | TR-06 |

The fourth row is the design point. **Out of domain is a successful response
carrying a refusal**, not an error. An error code would let a caller treat it as a
transient failure and retry; a 200 with `out_of_domain` and a null triage score
forces the caller to handle the honest answer (G5).

### 5.2 Concurrency

`admet_infer::Engine::run` takes `&mut self`, because an ONNX Runtime session
binds input tensors before executing. So the session cannot be shared, and the
scaffold wraps it in a `Mutex` — which is correct, and is also **a bottleneck that
serialises every prediction**. It is documented where it is created, in
`crates/admet-api/src/state.rs`, rather than discovered under load.

Increment 2 replaces it with the design NFR-02 actually calls for: one dedicated
worker task owning the engine, fed by an `mpsc` channel, coalescing up to 64
waiting requests into a single batched call. That is faster than a lock *and*
removes locking at this layer — micro-batching is a performance feature, not a
complication. TR-08 needs the same channel shape for batch ingestion, so the two
arrive together rather than as separate work.

---

## 6. Security design

### 6.1 Trust boundaries

Three, marked on Fig 7.2 and Fig 7.8:

1. **Browser → Caddy.** Everything past this point is untrusted input. TLS
   terminates here.
2. **Caddy → `admet-api`.** Loopback or an internal Docker network. Never exposed
   directly.
3. **`admet-api` → PostgreSQL.** Credentials from the environment only, never from
   a committed file.

The desktop build collapses all three: one process, no socket, and the operating
system's user account is the boundary.

### 6.2 STRIDE analysis

| Threat | Vector | Mitigation | State | Test |
|---|---|---|---|---|
| **S**poofing | No authentication on any route | Argon2id sessions (FR-23); loopback-only bind until then | **Open until Increment 3** — stated, not hidden | TC-S-010 |
| **T**ampering | Malicious SMILES crafting a huge graph | 1,000-char cap (FR-03) and 128-atom cap (FR-04), both **before** allocation | Present | TC-S-003, TC-U-004 |
| **T**ampering | SQL injection | Parameterised queries only; no string-built SQL (TR-07) | Present by construction | CI `test` |
| **R**epudiation | No record of who predicted what | `predictions` rows carry a timestamp and model version; user attribution from FR-23 | Partial | TC-I-052 |
| **I**nformation disclosure | Database password in logs | Hand-written `Debug` on `AppState` and on `admet_db::model::User`; the URL is never logged on failure | Present, and unit-tested | TC-S-020 |
| **I**nformation disclosure | Secrets committed | Environment-only secrets; pre-commit hook scans staged diffs for credential shapes; `.env` blocked from staging | Present | hook, manually exercised |
| **I**nformation disclosure | Verbose errors leaking internals | RFC 9457 bodies carry a typed `title` and a safe `detail`; no source paths, no SQL | Present | TC-I-005 |
| **D**enial of service | Huge request body | 20 MB limit **outside** the timeout layer (TR-06) | Present | TC-S-001 |
| **D**enial of service | Slow request holding a worker | 30 s timeout (TR-06) | Present | TC-S-002 |
| **D**enial of service | 50,000-row CSV materialised in memory | Bounded channel with backpressure (TR-08, R7) | Increment 4 | TC-P-005 |
| **E**levation of privilege | Cross-account project access | Every project query filtered by session user; no route trusts a client-supplied user id | Increment 3 | TC-S-014 |

Two rows deserve emphasis rather than a table cell:

**The spoofing row is open, and that is a decision with a date on it.** Until
Increment 3 the mitigation is the bind address: `127.0.0.1`, so the service is not
reachable from the network at all. `ADMET_SERVER__HOST=0.0.0.0` on a shared
network before FR-23 lands makes `/predict` available to every device on it. This
is written in three places — [`.env.example`](../.env.example),
[`config/default.toml`](../config/default.toml), and the header of
`crates/admet-api/src/main.rs` — because a warning in one place is a warning that
gets missed.

**The `Debug` mitigation is easy to lose in a refactor.** A derived `Debug` on
`AppState` would print the connection string into the first log line that formats
the state. That is why there is a test asserting the password does not appear in
the formatted output, rather than a comment asking future readers to be careful.

### 6.3 Input validation order

Order matters, and cheapest-first is not merely an optimisation — it is what keeps
an attacker from making the server do expensive work:

```text
1. Length ≤ 1,000 chars          FR-03   O(1), before any allocation
2. Lex                            FR-02   single pass, byte offsets retained
3. Parse to MolGraph              FR-02   typed errors, never a panic (NFR-06)
4. Heavy-atom count ≤ 128         FR-04   rejects, never truncates
5. Canonicalise, compute InChIKey FR-05   the cache key exists only now
6. Cache probe                    TR-05
7. Featurise, infer               FR-08
```

Step 4 sits after parsing because the atom count is not knowable before it, and
step 1 sits first because it is the only check that costs nothing. A design that
canonicalised before counting atoms would do the expensive work on inputs it was
about to reject.

## 7. Performance design

Four levers, in the order they are expected to matter. The point of ordering them
is that the sequence is a **prediction to be tested**, not a plan to be executed —
optimisation without measurement is guessing with extra steps.

| # | Lever | Expected effect | How it is verified |
|---|---|---|---|
| 1 | Cache hits on `(inchikey, model_version)` | Largest, on realistic batches with repeated structures | `just bench` with and without the cache |
| 2 | Micro-batching to 64 | Amortises ORT call overhead across requests | criterion sweep over batch size; 64 is claimed to be the knee, and the sweep is what makes that a fact |
| 3 | `intra_threads = 1` | Avoids thread oversubscription against request-level parallelism | latency at fixed concurrency, 1 vs 2 vs 4 |
| 4 | LTO and codegen-units | Smallest, and last | `lto = "thin"` today; a before/after against `"fat"` is the deliverable |

Lever 4 is deliberately unset. The build manual specifies `lto = "fat"`; this
project ships `"thin"` so that there is a real before/after number to report
instead of an assumed one ([ADR-07](adr/0007-native-windows-and-training-layout.md)).

Every figure produced by these levers lands in `results/` and is quoted from
there. None of them are in this document, because a performance number written in
prose is a number nobody can regenerate.

---

## Appendix A — Revision history

| Version | Date | Change |
|---|---|---|
| 0.1 | 2026-08-27 | Structure baselined. Eight figures specified. STRIDE table populated with current state per threat. |
