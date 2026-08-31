# 01 — Software Requirements Specification

| | |
|---|---|
| **System** | ADMETriage — explainable ADMET screening for small molecules |
| **Document** | SRS, structured per IEEE 830-1998 |
| **Status** | Baselined for Increment 1. Requirement *text* is frozen; verification columns fill in as tests land. |
| **Version** | 0.1 (scaffold) |
| **Author** | tusharbeckham |

## How this document relates to `requirements.md`

[`requirements.md`](../requirements.md) is the **register**: the shortest correct
statement of every identifier, and the single source of truth for what an ID
means. This document is the **specification**: the same requirements in IEEE 830
order, each with the context, the fit criterion, and the test that discharges it.

When the two disagree, the register wins on *wording* and this document wins on
*verification detail*. Neither renumbers: an ID, once issued, is permanent —
`git log --grep`, the test names, and
[`04-traceability.md`](04-traceability.md) all depend on that.

Cells written `→ §X` or `→ results/…` are **markers, not omissions**. They name
where a number must come from so that it cannot be invented later. A marker still
present at submission is an unfinished requirement, and it is meant to be visible.

---

## 1. Introduction

### 1.1 Purpose

This SRS states what ADMETriage must do, how well, and how each claim is checked.

It has three audiences, and each one settles a different question with it:

- **The implementer** (also the author) — is this behaviour in scope, and which
  increment owns it?
- **The assessor** — is the system that was built the system that was specified,
  and is every claim backed by something executable?
- **The maintainer, six months on** — why is it this way, and what breaks if I
  change it? That question is answered by the ADRs, which this document links
  rather than restates.

### 1.2 Scope

ADMETriage takes a molecule written as a SMILES string and returns, in one
response: deterministic physicochemical descriptors, predictions for **twelve**
ADMET endpoints, a per-prediction applicability-domain judgement, and an
atom-level explanation of what drove each alert.

It is delivered as a web application and, from the same codebase, an offline
desktop build.

**Objectives** (register §2):

| ID | Goal | Discharged mainly by |
|---|---|---|
| G1 | Predict twelve ADMET endpoints from a SMILES string alone | FR-08, [ADR-06](adr/0006-tdc-over-moleculenet.md) |
| G2 | Return a single-molecule report in under 300 ms at p95 | NFR-01 |
| G3 | Screen a 10,000-molecule library without manual intervention | UC-02, NFR-02 |
| G4 | Explain each prediction at the level of individual atoms | FR-19, FR-20 |
| G5 | State honestly when a molecule lies outside the model's competence | FR-10, FR-11, FR-12 |
| G6 | Run entirely locally, offline, at zero per-request cost | NFR-09, [ADR-01](adr/0001-rust-serving-onnx-boundary.md) |
| G7 | Ship as web and desktop from one codebase | FR-24, [ADR-02](adr/0002-hexagonal-crate-split.md) |
| G8 | Be documented to a standard that makes the engineering defensible | this document, the ADRs, [04-traceability.md](04-traceability.md) |

**Out of scope**, verbatim from register §8 and not negotiable inside v1: no
regulatory-grade toxicology claim, no molecule generation, no 3D conformers or
docking, no multi-tenant organisations or roles, no in-browser structure editor,
no retraining from user data.

The first of those is a **UI constraint, not a disclaimer**: no screen, export or
API field may be worded so that a screening estimate reads as an assay result.

### 1.3 Definitions

The full glossary is [`reference/glossary.md`](reference/glossary.md). Only the
terms this document leans on are defined here.

| Term | Meaning as used here |
|---|---|
| **Deterministic descriptor** | A quantity computed by an algorithm with one right answer — MW, cLogP, TPSA, HBD, HBA, rotatable bonds, ring count. Not a prediction. A mismatch against RDKit is a defect (NFR-05), never an error margin. |
| **Prediction** | A model output with irreducible uncertainty. Always carries a domain flag; never displayed in the same visual register as a descriptor (NFR-10). |
| **Heavy atom** | Any non-hydrogen atom. The 128 cap (FR-04) counts these. |
| **InChIKey** | 27-character hashed molecular identifier. Identity for deduplication and caching — see [ADR-04](adr/0004-inchikey-identity-and-cache-key.md). |
| **Applicability domain** | Whether a query resembles the training set closely enough for its prediction to mean anything. Operationalised as maximum Tanimoto similarity to the training set (FR-10). |
| **Bemis–Murcko scaffold** | The ring systems plus connecting linkers of a molecule, side chains stripped. The grouping key for the split — [ADR-05](adr/0005-scaffold-split-not-random.md). |
| **Desirability** | An endpoint's raw prediction mapped monotonically to `[0,1]`, so that twelve incommensurable units can be combined (FR-17). |
| **Golden fixture** | Committed input/output vectors that pin featuriser behaviour across two languages (TR-03). |

### 1.4 References

| Ref | Document |
|---|---|
| [R-01] | `ADMETriage_Build_Manual.pdf` — the build manual this project follows |
| [R-02] | [`requirements.md`](../requirements.md) — requirement register (authoritative IDs) |
| [R-03] | [`research.md`](../research.md) — literature and prior-art survey |
| [R-04] | [`method.md`](../method.md) — model and training methodology |
| [R-05] | [`implementation.md`](../implementation.md) — increment plan and tags |
| [R-06] | [`docs/adr/`](adr/) — ADR-01 … ADR-07 |
| [R-07] | [`docs/06-data-sources.md`](06-data-sources.md) — datasets and licence obligations |
| [R-08] | IEEE 830-1998 — SRS structure used here |
| [R-09] | RFC 9457 — Problem Details for HTTP APIs (TR-09) |
| [R-10] | Therapeutics Data Commons, ADMET benchmark group — the twelve endpoints |
| [R-11] | ONNX operator sets — opset 17 (TR-01) |

### 1.5 Overview

§2 describes the system as a whole and apportions requirements to increments. §3
is the specification proper: interfaces, then FR, then performance, then design
constraints, then system attributes. §4 defines the test-identifier scheme. §5 is
the open-question register — what is knowingly undecided.

---

## 2. Overall description

### 2.1 Product perspective

ADMETriage is self-contained. It calls no external service at request time (G6,
NFR-09), which is the property that makes it usable on unpublished structures.

Two processes and one file:

```text
 Python (training, offline)          Rust (serving, on the request path)
 ─────────────────────────           ──────────────────────────────────
 TDC → clean → scaffold split        admet-core    chemistry, no I/O
   → featurise (33-dim)              admet-infer   ONNX Runtime
   → train dense GIN                 admet-db      PostgreSQL 16
   → export ────────► model.onnx ──► admet-api     HTTP, RFC 9457
                      + feature_      admet-cli    same core, no server
                        schema.json
```

`model.onnx` plus `feature_schema.json` is the **entire** interface between the
two languages ([ADR-01](adr/0001-rust-serving-onnx-boundary.md)). Nothing else
crosses; no Python runs to answer a request (TR-02).

The system context is Fig 7.1 and the container view Fig 7.2 —
see [`diagrams/README.md`](diagrams/README.md) for what each must show.

### 2.2 Product functions

| ID | Use case | Actor | Increment |
|---|---|---|---|
| UC-01 | Predict properties for a single molecule | Chemist | 2–3 |
| UC-02 | Screen a CSV library in batch | Chemist | 4 |
| UC-03 | Rank and triage candidates by composite score | Project lead | 4 |
| UC-04 | Inspect per-atom attribution for one prediction | Chemist | 4 |
| UC-05 | Compare a candidate against approved-drug reference distributions | Chemist | 4 |
| UC-06 | Export a report as CSV or PDF | Project lead | 4 |
| UC-07 | Manage projects and saved molecule sets | Chemist | 3 |
| UC-08 | Operate the system offline via the desktop build | Chemist | 5 |

UC-01 is the vertical slice. It is built first, end to end, on one endpoint
(`BBB_Martins`, 1,975 rows) before breadth is added — see
[`implementation.md`](../implementation.md) §7. Every other use case reuses its
pipeline, so a defect there is found once rather than twelve times.

### 2.3 User characteristics

| | Chemist (primary) | Project lead (secondary) |
|---|---|---|
| Domain knowledge | High. Reads cLogP and TPSA without a legend. | Moderate to high. |
| Software expectations | A web form. Will not install Python. Will not read a stack trace. | The same, plus an export that survives being pasted into a slide. |
| Tolerance for a wrong answer | **Zero for descriptors**, which is why NFR-05 is exact equality. Non-zero for predictions **provided the uncertainty is stated** — hence G5. | Same, plus needs the caveat to survive into the exported artefact. |
| What loses their trust instantly | A number with no uncertainty that later proves wrong. | A report that cannot be defended in a meeting. |

That last row is the design driver behind NFR-10 and FR-11: a confidently wrong
prediction is worse than a stated refusal, because it is acted on.

### 2.4 Constraints

| # | Constraint | Consequence |
|---|---|---|
| C-1 | **No regulatory claim.** Screening aid only. | Wording constraint on every surface, including PDF export. |
| C-2 | **CPU only.** No GPU at train or serve time. | Caps model size; ~37,000 rows total makes this workable ([ADR-03](adr/0003-dense-adjacency-over-sparse-scatter.md)). |
| C-3 | **Fifteen weeks, one developer.** | Increments ordered by defensibility; 1–3 alone must present as complete (R9). |
| C-4 | **Offline at request time.** | Rules out hosted inference and paid APIs. |
| C-5 | **Development host is native Windows 11 + Git Bash**; CI and production are Linux. | A platform gap exists by choice — [ADR-07](adr/0007-native-windows-and-training-layout.md). |
| C-6 | **Two Python environments.** PyTDC pins `rdkit<2024.3.1`; the project runs 2026.3.5. | TDC lives in `.venv-tdc` and does **zero chemistry** ([ADR-06](adr/0006-tdc-over-moleculenet.md)). |
| C-7 | **`ort` is pinned to an exact pre-release.** | R6. The dependency is wrapped behind one type in `admet-infer`. |
| C-8 | **Licence obligations of the training data** must be honoured in any distribution. | See [06-data-sources.md](06-data-sources.md) §licences. |

### 2.5 Assumptions and dependencies

| # | Assumption | If it proves false |
|---|---|---|
| A-1 | TDC remains downloadable and its ADMET group stable. | Datasets are cached locally under `data/raw/`; a hash manifest makes a silent upstream change detectable rather than invisible. |
| A-2 | RDKit is the correct oracle for deterministic descriptors. | NFR-05 becomes meaningless; the descriptor contract would need a different reference. Accepted: RDKit *is* the field's reference implementation. |
| A-3 | A dense 128×128 adjacency is exportable to ONNX opset 17 and fast enough. | Verified by spike **before** any Rust was written (R1). Evidence: `training/scripts/spike_onnx_export.py`. |
| A-4 | Endpoints with <700 rows can learn something useful via a shared trunk. | R2 — reported honestly per endpoint rather than hidden in a mean. |
| A-5 | PostgreSQL 16 is available in dev, CI and production; SQLite suffices for desktop. | FR-24 assumes a schema portable between them; keeping `admet-core` I/O-free is what makes that swap cheap. |

### 2.6 Apportioning of requirements

| Tag | Increment | Requirements |
|---|---|---|
| `v0.0.1-scaffold` | 0 — Scaffold | TR-10, TR-11 (partially: CI green, tags in place) |
| `v0.1.0-model` | 1 — Model core | FR-05, TR-01, TR-03, TR-12, NFR-03, NFR-07 |
| `v0.2.0-api` | 2 — Inference service | FR-01–08, FR-10, FR-11, TR-02, TR-04–09, NFR-01, NFR-05, NFR-06 |
| `v0.3.0-web` | 3 — Web workspace | FR-09, FR-13, FR-23, NFR-10 |
| `v0.4.0-batch` | 4 — Batch + explainability | FR-12, FR-14–22, NFR-02 |
| `v0.5.0-desktop` | 5 — Packaging | FR-24, TR-10, TR-11, NFR-11, NFR-12 |

This apportionment is copied from [`implementation.md`](../implementation.md) §4
and must stay identical to it. If they drift, the tag is right and this table is
stale, because the tag is the artefact that exists.

---

## 3. Specific requirements

### 3.1 External interface requirements

#### 3.1.1 User interfaces

Web (SvelteKit) and desktop (Tauri 2) render the *same* components against the
same response shapes. The desktop build calls `invoke` where the web build calls
`fetch`; nothing above that line differs.

Two rules bind every screen:

- **NFR-10 is visual, not textual.** A computed descriptor and a predicted value
  must be distinguishable at a glance — different container, not a footnote.
  A caveat nobody reads is not a caveat.
- **A withheld number is rendered as withheld** (FR-12), never as `0`, `—`, or a
  blank cell. Blank reads as "not applicable"; the truth is "refused, and here is
  why".

#### 3.1.2 Application programming interface

JSON over HTTP. Errors are RFC 9457 problem details (TR-09) with `type`, `title`,
`status`, `detail`, `instance`, plus a `position` extension member carrying the
byte offset for SMILES faults (FR-02).

| Method & path | Purpose | Requirements | Increment |
|---|---|---|---|
| `GET /healthz` | Liveness. Answers whenever the process is up. | — | 0 ✅ |
| `GET /readyz` | Readiness. 503 while the model or database is absent. | NFR-11 | 0 ✅ |
| `GET /version` | Build, model version, uptime. | TR-10 | 0 ✅ |
| `POST /predict` | One molecule → descriptors + twelve endpoints. | FR-01–11 | 2 |
| `POST /batch` | CSV upload, returns a job handle. | FR-14, TR-08 | 4 |
| `GET /batch/{id}/events` | Server-sent progress stream. | FR-15 | 4 |
| `GET /batch/{id}` | Results, ranked, paginated. | FR-16, FR-18 | 4 |
| `GET /predict/{inchikey}/attribution` | Per-atom scores. | FR-19 | 4 |
| `POST /export` | CSV or PDF. | FR-22 | 4 |
| `POST /auth/login`, `/auth/logout` | Argon2id sessions. | FR-23 | 3 |
| `GET /projects`, `POST /projects` | Named sets. | FR-23 | 3 |

✅ = exists in the scaffold. The OpenAPI document is generated from the Rust
types, never hand-written, and `web/src/lib/types.ts` is generated from it — one
definition, two consumers.

#### 3.1.3 Hardware interfaces

None specific. Target envelope: 2 vCPU, 2 GB RAM (NFR-02, NFR-12). No GPU, no
accelerator, no assumption about instruction-set extensions beyond the
`x86_64` baseline.

#### 3.1.4 Software interfaces

| Interface | Contract | Enforced by |
|---|---|---|
| `models/model.onnx` | opset 17; inputs `x[B,128,33] f32`, `adj[B,128,128] f32`, `mask[B,128] f32`; output `y[B,12] f32`; **only** the batch axis dynamic. | TR-01, asserted at load in `admet-infer` |
| `models/feature_schema.json` | Generated **from** Rust (`admet_core::features::schema_json()`), asserted **by** Python. Carries `schema_version`, `n_features`, `max_heavy_atoms`, per-block widths summing to 33. | TR-03; version mismatch is a refusal to start, not a warning |
| PostgreSQL 16 | `sqlx`, runtime-checked queries plus a committed `.sqlx/` for offline builds. | TR-07 |
| SQLite (desktop) | Same logical schema, bundled read-write file. | FR-24 |
| RDKit | Oracle for descriptors and InChIKey. | NFR-05 |

The direction of the schema dependency is deliberate and is the whole of R3's
mitigation: **Rust emits, Python asserts.** Reversed, a Python-side change would
silently redefine the contract and the model would train on features the server
never produces.

#### 3.1.5 Communications interfaces

HTTP/1.1 and HTTP/2 over TLS in production (TLS terminated by Caddy in front).
CORS is an explicit origin list, never `*`, because a session cookie cannot be
sent to a wildcard origin. Batch progress uses server-sent events rather than
WebSockets: the traffic is one-directional, and SSE reconnects on its own.

### 3.2 Functional requirements

The register (§5 of [`requirements.md`](../requirements.md)) holds the canonical
one-line statement of each FR. This section adds the **fit criterion** — the
observable condition that decides pass or fail — and the test that checks it.

`Verified by` cites the scheme in §4. An entry that still reads `→ TBD` is a
requirement without a test, and is a defect in this document.

#### 3.2.1 Input handling and validation (UC-01)

| ID | Fit criterion | Verified by |
|---|---|---|
| FR-01 | A valid SMILES submitted by web form and by `POST /predict` produce byte-identical prediction bodies. | TC-SYS-001, TC-I-001 |
| FR-02 | Every rejection names a **byte offset** into the input. `C1CC` → unclosed ring at position 0; `C(C` → unclosed branch at 1; `CX` → unknown element at 1. The offset is a byte index, not a character index, and the API returns it in `position`. | TC-U-010 … TC-U-024, TC-I-004 |
| FR-03 | Input longer than 1,000 characters is rejected **before** parsing, with a length error rather than a parse error. | TC-U-002, TC-S-003 |
| FR-04 | A molecule with >128 heavy atoms is **rejected with a reason**, never truncated. Boundary: 128 accepted, 129 rejected. | TC-U-003, TC-U-004 |
| FR-05 | The InChIKey is exactly 27 characters and identical for the three spellings of aspirin in [ADR-04](adr/0004-inchikey-identity-and-cache-key.md). | TC-U-030, TC-U-031 |

FR-04 is the requirement most likely to be "helpfully" softened, so it is stated
twice: **truncating a molecule produces a different molecule**, and a prediction
for a different molecule is not a degraded answer, it is a wrong one.

#### 3.2.2 Deterministic chemistry (UC-01)

| ID | Fit criterion | Verified by |
|---|---|---|
| FR-06 | MW, cLogP, TPSA, HBD, HBA, rotatable bonds and ring count match RDKit **exactly** (NFR-05) over a 500-molecule reference set. Not "within tolerance". | TC-U-040, TC-I-006 |
| FR-07 | The response lists Lipinski and Veber violations **individually and named**. A count alone fails this requirement even if the count is right. | TC-U-045, TC-SYS-004 |

#### 3.2.3 Prediction and honesty (UC-01, G5)

| ID | Fit criterion | Verified by |
|---|---|---|
| FR-08 | One `POST /predict` returns all twelve endpoints. Eleven is a failure; twelve with one `null` and a stated reason is a pass. | TC-I-010 |
| FR-09 | Each endpoint carries a band (low / moderate / high) derived from a documented threshold, plus the raw value. Thresholds live in `method.md`, never in UI code. | TC-U-050, TC-SYS-006 |
| FR-10 | Maximum Tanimoto similarity to the training set, over the Morgan fingerprint defined in `admet-core`, returned per prediction. | TC-U-055, TC-U-056 |
| FR-11 | `low_confidence` below **0.45**, `out_of_domain` below **0.30**. Boundary cases at exactly 0.45 and 0.30 are pinned by test, so a later refactor cannot silently move an inequality. | TC-U-057, TC-U-058 |
| FR-12 | An out-of-domain molecule returns `triage_score: null` with a machine-readable reason. The absence is explicit in both API and UI. | TC-U-060, TC-SYS-007 |
| FR-13 | The 2D depiction is of the **parsed** molecule, so a user can confirm the system read what they meant. | TC-SYS-008 |

#### 3.2.4 Batch screening (UC-02, UC-03)

| ID | Fit criterion | Verified by |
|---|---|---|
| FR-14 | 50,000 rows accepted; 50,001 rejected with a stated limit. Invalid rows are skipped and **reported**, never dropped silently. | TC-I-020, TC-S-005 |
| FR-15 | Progress is observable while the job runs — an SSE event at least every 250 rows. A batch that reports only on completion fails. | TC-I-022 |
| FR-16 | Results survive a page reload and a server restart mid-job: `completed_rows` is checkpointed, so a crash at row 9,000 resumes rather than restarts. | TC-I-023, TC-SYS-010 |
| FR-17 | Composite triage is the **geometric** mean of per-endpoint desirabilities. Verified against a hand-computed example; the arithmetic mean must fail the test. | TC-U-070, TC-U-071 |
| FR-18 | Top-k is stable and ranked by triage score, with a documented tie-break. `NULL` scores sort last under the explicit `NULLS LAST`, not by accident. | TC-I-025 |

FR-17's fit criterion names the wrong answer on purpose. A geometric mean is
zero if any factor is zero — one disqualifying endpoint sinks the candidate — and
that is exactly the behaviour wanted. An arithmetic mean lets eleven good scores
hide one fatal one, which is how a triage tool becomes a liability.

#### 3.2.5 Explanation, comparison, export (UC-04 … UC-06)

| ID | Fit criterion | Verified by |
|---|---|---|
| FR-19 | Per-atom attribution has one score per heavy atom, in the input's atom order, and the mapping back to input atom indices is tested — not assumed. | TC-U-080, TC-I-030 |
| FR-20 | The overlay colours the atoms the attribution names. Verified by asserting the DOM/canvas mapping, not by eye. | TC-SYS-012 |
| FR-21 | Comparison is against a documented approved-drug reference set with a recorded revision, so the baseline is reproducible. | TC-I-032 |
| FR-22 | CSV round-trips into a spreadsheet without mangling; the PDF carries the domain caveat (C-1) and the model version. | TC-SYS-014, TC-SYS-015 |

#### 3.2.6 Projects and offline operation (UC-07, UC-08)

| ID | Fit criterion | Verified by |
|---|---|---|
| FR-23 | Argon2id password hashing with per-user salt; a session cookie that is `HttpOnly`, `Secure`, `SameSite=Lax`. No project is readable across accounts. | TC-S-010 … TC-S-014 |
| FR-24 | With the network interface disabled, the desktop build performs UC-01 end to end against a bundled SQLite database and an embedded `model.onnx`. | TC-SYS-020 |

FR-24's test is worth stating as written: **disable the interface, then run it.**
"No network calls in the code" is an inspection; pulling the cable is a test.

### 3.3 Performance requirements

Every cell in the **Measured** column is a marker until a script in this
repository fills it. The rule is absolute: a target is not a measurement, and a
missed target is recorded as a miss rather than edited into agreement.

| ID | Target | Measured how | Measured value |
|---|---|---|---|
| NFR-01 | Single prediction p95 < 300 ms, p99 < 600 ms, warm cache | `just bench` → `oha`/`k6` against `POST /predict`, 60 s, warm | → `results/latency.json`, reported in §Performance of the final report |
| NFR-02 | 10,000-molecule batch < 90 s on 2 vCPU | `just bench-cli N=10000` with the container limited to 2 CPUs | → `results/throughput.json` |
| NFR-03 | Mean AUROC ≥ 0.80 across classification endpoints, scaffold-held-out | `just results`, 5 seeds, mean ± sd, **both** split strategies reported | → `results/metrics.json` |
| NFR-04 | Coverage ≥ 75% on `admet-core` and `admet-infer` | `cargo llvm-cov --workspace` in CI | → CI artefact per run |
| NFR-11 | Cold start < 10 s including model load | Time from process start to first 200 on `/readyz` | → `results/startup.txt` |
| NFR-12 | < 512 MB resident under batch load; desktop installer < 15 MB | `docker stats` peak during the NFR-02 run; `ls -l` on the bundle | → `results/footprint.txt` |

Three notes that decide whether these numbers mean anything:

- **"Warm cache" is part of the NFR-01 target, so cold latency must also be
  reported.** Quoting only the warm figure is the most common way a latency claim
  becomes untrue in practice.
- **NFR-03 is a mean over endpoints, and a mean hides an endpoint that failed.**
  Per-endpoint values are reported alongside it (R2). An endpoint below 0.70 is
  discussed, not averaged away.
- **NFR-02's 2 vCPU is a constraint on the measurement, not a footnote.** A
  throughput number from an unconstrained laptop is not evidence for it.

### 3.4 Design constraints

The technical requirements are constraints in the IEEE 830 sense: they narrow the
design space before implementation begins, each for a stated reason.

| ID | Constraint | Why, and where argued | Verified by |
|---|---|---|---|
| TR-01 | ONNX opset 17, only the batch axis dynamic | A second dynamic axis defers shape inference to runtime and turns a build-time error into a wrong-shape failure in production. [ADR-03](adr/0003-dense-adjacency-over-sparse-scatter.md) | TC-U-100, spike script |
| TR-02 | ORT in Rust; no Python on the request path | GIL and GC pauses land in the tail, which is what users feel. [ADR-01](adr/0001-rust-serving-onnx-boundary.md) | TC-P-001, inspection |
| TR-03 | Featuriser identical in both languages, golden fixture to **1e-6** | R3. Skew yields plausible wrong answers, not crashes. | TC-U-090, TC-U-091 |
| TR-04 | End-to-end parity to **1e-4** in CI | Catches drift anywhere in the chain, not just the featuriser. | TC-I-050 (`onnx_parity`) |
| TR-05 | Cache keyed `(InChIKey, model_version)` | A model upgrade invalidates **by construction** rather than by remembering to flush. [ADR-04](adr/0004-inchikey-identity-and-cache-key.md) | TC-U-035, TC-I-052 |
| TR-06 | 20 MB body limit, 30 s timeout | Memory exhaustion and connection exhaustion are the two cheapest attacks on this shape of service. | TC-S-001, TC-S-002 |
| TR-07 | `sqlx` checked queries on PostgreSQL 16 | A typo in SQL becomes a compile error instead of a 500 at 2 a.m. | CI `test` job |
| TR-08 | Bounded-memory batch ingestion: a streaming channel, not a materialised `Vec` | R7. 50,000 rows materialised is the OOM. | TC-P-005, TC-I-021 |
| TR-09 | RFC 9457 problem details | One error shape for browser, CLI and desktop; the `position` member is what makes FR-02 usable. | TC-I-005 |
| TR-10 | Every increment tagged, every tag runnable | An untagged increment cannot be demonstrated later. | `git tag` at each increment |
| TR-11 | CI runs fmt, clippy, test, coverage, audit, licence on every push | A gate that runs on request is a gate that does not run. | `.github/workflows/ci.yml` |
| TR-12 | Seeded runs; split deterministic per dataset revision | NFR-07, R10. A metric that cannot be regenerated cannot be defended. | TC-U-095, `just results` |

### 3.5 Software system attributes

**Reliability (NFR-06).** No input may panic the service. The parser is written
against this: `Result` everywhere on the input path, `unwrap` forbidden outside
tests, and a property test (`proptest`) that feeds arbitrary bytes to the parser
and asserts only that it returns rather than aborts. Rationale: a panic in a
request handler is a 500 with no problem body, so it defeats TR-09 as well.

**Availability.** `/healthz` answers whenever the process lives; `/readyz` fails
while the model or the database is absent. A degraded start is deliberate — a
container that exits on a missing artefact crash-loops and takes its diagnostics
with it, whereas one that starts and reports `model: absent` is inspectable with
a single `curl`. **No prediction is ever served from a degraded state**: `/predict`
without an engine is a typed 503, never a default value.

**Statelessness (NFR-08).** `POST /predict` holds no per-user session state. Every
request carries everything needed to answer it, and the only server-side state is
the prediction cache — which is keyed on `(inchikey, model_version)`
([ADR-04](adr/0004-inchikey-identity-and-cache-key.md)) and is therefore shared,
not per-caller, and safe to drop at any moment. Two consequences that are the
reason this is a requirement rather than an implementation note: a second instance
can be added behind a load balancer without sticky sessions, and a restart loses
nothing but warmth. Verified by TC-I-005, which issues the same request to two
independently constructed `AppState`s and asserts identical responses.

> Added late. This requirement was present in the register and cited in
> [`04-traceability.md`](04-traceability.md), but had no definition anywhere in
> this document — so a reader working from the repository alone could not find out
> what NFR-08 required. Found by `scripts/check-traceability.py` (DEF-17).

**Security.** Full STRIDE analysis in [`02-design.md`](02-design.md) §Security.
The posture *today*, stated plainly rather than implied:

| Control | State in the scaffold |
|---|---|
| Authentication | **None on any route.** Arrives with FR-23 in Increment 3. |
| Bind address | `127.0.0.1` by default, so the service is not reachable off-box. Do **not** set `0.0.0.0` before Increment 3. |
| Body limit / timeout | Present now (TR-06). |
| CORS | Explicit origin list, never `*`. |
| Secrets | Environment only. `config/default.toml` carries a placeholder that fails loudly; the pre-commit hook scans staged diffs for credential shapes. |
| SQL injection | Parameterised queries only (TR-07). |
| Password storage | Argon2id with per-user salt, from FR-23. |

**Maintainability.** NFR-04 sets the coverage floor on the two crates where a
defect is silent rather than loud. `admet-core` has **no I/O dependencies**
([ADR-02](adr/0002-hexagonal-crate-split.md)), which is what lets the pre-commit
hook run its whole suite in about a second — a gate that takes a minute gets
skipped, and a skipped gate is not a gate.

**Portability (G7, NFR-09, NFR-12).** One codebase, two shipped forms. The
desktop build links `admet-core` and `admet-infer` directly and swaps HTTP for
`invoke`; if the domain crate held a `PgPool`, that build would be a rewrite
instead of a wrapper.

### 3.6 Other requirements

Dataset licence obligations, per-asset, are in
[`06-data-sources.md`](06-data-sources.md). Two carry through into the product
rather than staying a data concern: attribution must appear in the exported PDF,
and any redistribution of the training data must repeat its original terms.

---

## 4. Verification scheme

Test identifiers are stable and referenced from three places: the test name in
code, this document, and [`04-traceability.md`](04-traceability.md).

| Prefix | Level | Lives in | Runs in |
|---|---|---|---|
| `TC-U-nnn` | Unit — one function, no I/O | `crates/*/src/**` inline `#[cfg(test)]`, `training/tests/` | pre-commit and CI |
| `TC-I-nnn` | Integration — crates together, real PostgreSQL | `crates/*/tests/` | CI (`test` job, with the `postgres:16-alpine` service) |
| `TC-SYS-nnn` | System — HTTP or browser, end to end | `crates/admet-api/tests/`, `web/tests/` | CI, and manually before a tag |
| `TC-P-nnn` | Performance — a number, not a pass/fail | `crates/*/benches/`, `just bench` | on demand; recorded in `results/` |
| `TC-S-nnn` | Security — one STRIDE threat each | alongside the level it exercises | CI |

The convention that makes this searchable: **the test's own name contains its ID
in a comment**, so `git grep TC-U-090` reaches the code and `git log --grep=FR-04`
reaches the history. Numbering leaves gaps deliberately — inserting a test next to
its neighbour must not renumber anything.

## 5. Open questions

Recorded because an undecided question that is written down is a task, and one
that is not is a surprise. Each must be closed before the increment that needs it.

| # | Question | Needed by | Current lean |
|---|---|---|---|
| Q-1 | **Opset 17 or 18?** TR-01 says 17; `spike_onnx_export.py` sets `OPSET = 17`; commit `fad0660` says "verified … at opset 18". One of the three is wrong. | Increment 1, before training | Settle on 17 (TR-01), re-run the spike, and amend whichever record is wrong. Do not leave two numbers in the repository. |
| Q-2 | **Salt and tautomer policy before the InChIKey.** ADR-04 states the cost: tautomers hash differently, and a salt changes the key. Which normalisation runs first? | Increment 1 (the key is computed there) | Strip counter-ions to the largest organic fragment; do **not** canonicalise tautomers. Record the choice in ADR-04 as an amendment and state the limitation in the report. |
| Q-3 | **Calibration method for FR-09's bands.** Isotonic, Platt, or documented fixed thresholds? | Increment 3 | Fixed, documented thresholds from `method.md` for v1; calibration needs a held-out set the small endpoints cannot spare. |
| Q-4 | **PDF engine for FR-22.** Typst, headless-Chrome print, or a Rust PDF crate? | Increment 4 | Typst — already a project dependency for the report, and it renders offline, which FR-24 needs. |
| Q-5 | **Approved-drug reference set for FR-21**, with a citable revision. | Increment 4 | A frozen subset of DrugBank approved small molecules, hash-pinned like the TDC data. |

---

## Appendix A — Revision history

| Version | Date | Change |
|---|---|---|
| 0.1 | 2026-08-27 | Baselined for Increment 1. All 24 FR, 12 TR, 12 NFR, 8 UC imported from the register with fit criteria and verification markers. Open questions Q-1 … Q-5 opened. |

## Appendix B — What must change in this document, and when

| Trigger | Update |
|---|---|
| A test lands | Replace the matching `Verified by` cell with the real ID. |
| A number is measured | Fill the **Measured value** cell in §3.3 and link the artefact in `results/`. |
| An open question closes | Move it out of §5 into the ADR or section that now owns it. |
| A requirement changes meaning | **New ID.** Never re-word an existing one — the history and the tests reference it. |
| An increment is tagged | Confirm §2.6 still matches `implementation.md`; the tag wins. |
