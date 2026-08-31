# Build-manual index — chapter → repository path

| | |
|---|---|
| **System** | ADMETriage |
| **Document** | Navigation aid for `ADMETriage_Build_Manual.pdf` (114 pp., 35 chapters) |
| **Status** | Built from the citations that exist in this repository. Unmapped rows are marked. |
| **Traces** | every `//! Manual chapter N` comment in `crates/` and `training/` |

## 1. What this is, and what it is not

The manual is 114 pages and the repository is several hundred files. The question that
comes up twenty times over fifteen weeks is **"which chapter covers the thing I am
about to write?"** — and answering it by scrolling a PDF costs a few minutes each time.

This index answers it in both directions: chapter → path, and path → chapter.

**It is derived, not transcribed.** Every row below comes from a `Manual chapter N`
citation that exists in a source file or document in this repository, found with:

```bash
git grep -nE "(Manual|manual) (chapter|ch\.) ?[0-9]+" -- crates training docs config Cargo.toml scripts
```

That matters for trust. A hand-typed table of contents drifts from the PDF silently;
this one can be regenerated, and a chapter that appears here **is** cited somewhere in
the code, which is a stronger statement than "the manual has a chapter about it".

Rows marked **not yet mapped** are chapters nothing in the repository cites yet. That
is a reading task, not an error — fill the row in when you get there, in the same commit
as the code that cites it.

## 2. Chapter → path

### Part One — Foundations (ch. 2–5)

Closed by the scaffold. Everything in this section exists.

| Ch. | Subject | Where it landed | Listings used |
|---|---|---|---|
| 2 | Machine setup, toolchain verification | [`00-machine-setup.md`](../00-machine-setup.md), [`scripts/verify-env.sh`](../../scripts/verify-env.sh), [`scripts/verify-env.ps1`](../../scripts/verify-env.ps1) | 2.8 (check harness), 2.10 |
| 3 | Data sourcing, licences, resource directory | [`06-data-sources.md`](../06-data-sources.md), [`training/data/download_tdc.py`](../../training/data/download_tdc.py) | 3.2 (download), Table 3.1 (endpoints), 3.3 |
| 4 | Monorepo scaffold, workspace, task runner | [`Cargo.toml`](../../Cargo.toml), [`justfile`](../../justfile), [`.env.example`](../../.env.example) | 4.2 (workspace deps), 4.3 (justfile), 4.4 (env), 4.5 (first tag) |
| 5 | Version-control discipline | [`05-git-conventions.md`](../05-git-conventions.md), [`.githooks/pre-commit`](../../.githooks/pre-commit), `.github/PULL_REQUEST_TEMPLATE.md` | 5.3 (hook), Table 5.1 (branches), Table 5.2 (self-review) |

### Part Two — The domain crate (ch. 6–15)

Increment 2's chemistry. Every module named here exists as a stub with typed signatures
and no body; the chapter is what fills it.

| Ch. | Subject | Where it lands | Listings used |
|---|---|---|---|
| 6 | Graph representation — SoA, CSR | [`admet-core/src/graph.rs`](../../crates/admet-core/src/graph.rs) | 6.2 (`MolGraph`) |
| 7 | SMILES parsing | [`admet-core/src/smiles/mod.rs`](../../crates/admet-core/src/smiles/mod.rs) | — |
| 7.1 | Lexer | [`smiles/lexer.rs`](../../crates/admet-core/src/smiles/lexer.rs) | — |
| 7.2 | LL(1) parser, byte-offset errors (FR-02) | [`smiles/parser.rs`](../../crates/admet-core/src/smiles/parser.rs) | 7.1 |
| 7.3 | Ring closure, SSSR via union-find | [`smiles/ring.rs`](../../crates/admet-core/src/smiles/ring.rs) | — |
| 8 | Canonicalisation and InChIKey (FR-05) | [`admet-core/src/canonical.rs`](../../crates/admet-core/src/canonical.rs), [`training/data/clean.py`](../../training/data/clean.py) | Table 8.2 (desalting rules) |
| 9 | Bemis–Murcko scaffolds | [`admet-core/src/scaffold.rs`](../../crates/admet-core/src/scaffold.rs), [`training/data/scaffold_split.py`](../../training/data/scaffold_split.py) | — |
| 10 | The 33-feature contract (TR-03) | [`admet-core/src/features.rs`](../../crates/admet-core/src/features.rs) | — |
| 11 | ER model, row types | [`admet-db/src/model.rs`](../../crates/admet-db/src/model.rs) | — |
| 12 | Normalisation decisions | [`admet-db/src/model.rs`](../../crates/admet-db/src/model.rs) (module docs) | — |
| 13 | **not yet mapped** | | |
| 14 | Triage scoring, desirability (FR-17) | [`admet-core/src/triage.rs`](../../crates/admet-core/src/triage.rs) | 14.1 (desirability mapping) |
| 15 | Fingerprints, applicability domain (FR-10, FR-11) | [`admet-core/src/fingerprint.rs`](../../crates/admet-core/src/fingerprint.rs) | — |

### Part Three — Model and service (ch. 16–22)

| Ch. | Subject | Where it lands | Listings used |
|---|---|---|---|
| 16 | **not yet mapped** — likely model architecture | `training/models/` | |
| 17 | **not yet mapped** | | |
| 18 | Increment 1: data acquisition and profiling | [`training/data/download_tdc.py`](../../training/data/download_tdc.py), [`training/data/profile.py`](../../training/data/profile.py) | 18.1 (`ENDPOINTS`, `profile()`), 18.2 step 1 |
| 19 | HTTP API — Axum | [`admet-api/src/lib.rs`](../../crates/admet-api/src/lib.rs), [`main.rs`](../../crates/admet-api/src/main.rs), [`routes/mod.rs`](../../crates/admet-api/src/routes/mod.rs) | — |
| 19.2 | Shared state | [`admet-api/src/state.rs`](../../crates/admet-api/src/state.rs) | — |
| 19.3 | `/predict` (FR-01, FR-08) | [`routes/predict.rs`](../../crates/admet-api/src/routes/predict.rs) | — |
| 19.4 | RFC 9457 problem details (TR-09) | [`admet-api/src/error.rs`](../../crates/admet-api/src/error.rs) | — |
| 20.3 | Repositories, bulk insert, checkpointing | [`admet-db/src/repository/`](../../crates/admet-db/src/repository/) — `mod`, `molecule`, `prediction`, `batch` | — |
| 21 | **not yet mapped** | | |
| 22 | CLI — `predict` / `import` / `bench` | [`admet-cli/src/main.rs`](../../crates/admet-cli/src/main.rs) | — |

### Part Four — Quality, performance, operations (ch. 23–30)

| Ch. | Subject | Where it lands | Listings used |
|---|---|---|---|
| 23 | Test strategy, pyramid, evidence | [`03-test-plan.md`](../03-test-plan.md), [`evidence/README.md`](../evidence/README.md) | Table 23.1 (pyramid), 23.4 (evidence folder) |
| 24 | Performance measurement | [`admet-core/benches/core.rs`](../../crates/admet-core/benches/core.rs) | 24.1 (criterion setup) |
| 24.2 | Build-profile optimisation — LTO, codegen units | [`Cargo.toml`](../../Cargo.toml) profile section, [ADR-07](../adr/0007-native-windows-and-training-layout.md) | — |
| 25 | Timing instrumentation | [`admet-core/src/lib.rs`](../../crates/admet-core/src/lib.rs) | 25.2 (nanosecond timing) |
| 26.2 | CI pipeline | `.github/workflows/ci.yml` | 26.2 (four jobs) |
| 26.3 | Layered configuration | [`config/default.toml`](../../config/default.toml), [`admet-api/src/config.rs`](../../crates/admet-api/src/config.rs) | 26.3 |
| 26.4 | Health and readiness probes | [`routes/health.rs`](../../crates/admet-api/src/routes/health.rs) | — |
| 26.5 | Structured tracing | [`admet-api/src/tracing_setup.rs`](../../crates/admet-api/src/tracing_setup.rs) | — |
| 27 | Diagrams — tools, consistency, export | [`diagrams/README.md`](../diagrams/README.md) | Table 27.1 (tools), Table 27.2 (consistency) |
| 28 | Traceability and cross-referencing | [`04-traceability.md`](../04-traceability.md) | Table 28.2 (matrix rows) |
| 29 | **not yet mapped** | | |
| 30 | **not yet mapped** | | |

### Appendices

| App. | Subject | Where it landed |
|---|---|---|
| A | Command reference | [`reference/commands.md`](commands.md) |
| B | **not yet mapped** | |
| C | Troubleshooting | [`reference/troubleshooting.md`](troubleshooting.md) |
| D | Glossary | [`reference/glossary.md`](glossary.md) |
| E | Journal template (Listing E.1) | [`journal/WEEK-TEMPLATE.md`](../journal/WEEK-TEMPLATE.md) |

## 3. Path → chapter

The reverse lookup, for when you are already in a file and want the chapter that
explains it. Regenerate with the `git grep` above; the `//! Manual chapter N` comment at
the top of each module is the source of truth.

| Path | Chapter |
|---|---|
| `crates/admet-core/src/graph.rs` | 6 |
| `crates/admet-core/src/smiles/{mod,lexer,parser,ring}.rs` | 7, 7.1, 7.2, 7.3 |
| `crates/admet-core/src/canonical.rs` | 8 |
| `crates/admet-core/src/scaffold.rs` | 9 |
| `crates/admet-core/src/features.rs` | 10 |
| `crates/admet-core/src/triage.rs` | 14 |
| `crates/admet-core/src/fingerprint.rs` | 15 |
| `crates/admet-core/src/lib.rs` | 25 (Listing 25.2) |
| `crates/admet-core/benches/core.rs` | 24, 24.1 |
| `crates/admet-db/src/{lib,model}.rs` | 11, 12 |
| `crates/admet-db/src/repository/*.rs` | 20.3 |
| `crates/admet-api/src/{lib,main}.rs`, `routes/mod.rs` | 19 |
| `crates/admet-api/src/state.rs` | 19.2 |
| `crates/admet-api/src/routes/predict.rs` | 19.3 |
| `crates/admet-api/src/error.rs` | 19.4 |
| `crates/admet-api/src/routes/health.rs` | 26.4 |
| `crates/admet-api/src/config.rs`, `config/default.toml` | 26.3 |
| `crates/admet-api/src/tracing_setup.rs` | 26.5 |
| `crates/admet-cli/src/main.rs` | 22 |
| `training/data/download_tdc.py` | 3.2, 3.3, 18.2 |
| `training/data/profile.py` | 18.2 (Listing 18.1) |
| `training/data/clean.py` | 8 (Table 8.2) |
| `training/data/scaffold_split.py` | 9 |
| `scripts/verify-env.{sh,ps1}` | 2, 2.10 (Listing 2.8) |
| `Cargo.toml` | 4.2, 24.2 |
| `justfile` | 4.3 |

## 4. One warning about figure numbers

**`Fig 7.x` in [`02-design.md`](../02-design.md) is not manual chapter 7.** The design
document numbers its eight figures 7.1–7.8 following the manual's *design-document*
template, while manual chapter 7 is SMILES parsing. Similarly, `report ch. N` in
[`diagrams/README.md`](../diagrams/README.md) refers to the final report's chapters, not
the manual's.

Three numbering systems coexist:

| When you see | It means |
|---|---|
| `Manual chapter 7` / `ch. 7` | the build manual |
| `Fig 7.5` | a figure in [`02-design.md`](../02-design.md) §2 |
| `report ch. 5` | a chapter of the final submitted report |

That collision is not fixable — both documents' numbering is fixed by their templates —
so the convention is to always write the qualifier. A bare "chapter 7" in a commit
message will be ambiguous in November.

## Appendix A — Revision history

| Version | Date | Change |
|---|---|---|
| 0.1 | 2026-08-27 | Built from the 40 `Manual chapter N` citations present at the scaffold tag. Chapters 13, 16, 17, 21, 29, 30 and Appendix B unmapped. |
