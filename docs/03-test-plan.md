# 03 — Test Plan

| | |
|---|---|
| **System** | ADMETriage |
| **Document** | Test plan, strategy, and defect log |
| **Status** | Strategy baselined. Counts are targets; the **Actual** column fills in per increment. |
| **Version** | 0.1 (scaffold) |
| **Traces to** | [`01-srs.md`](01-srs.md) §4, [`04-traceability.md`](04-traceability.md) |

## 1. What this plan is for

Two claims have to be defensible at the end of this project:

1. **Every requirement is checked by something executable.** Not "was tested" —
   *is* tested, on every push, by an artefact anyone can run.
2. **Every number quoted is a measurement.** The distinction between a target and
   a measurement is the single most important honesty rule in this repository, and
   the test plan is where it is enforced.

The first is discharged by the traceability matrix. The second is discharged by
`results/` and by refusing to write a figure anywhere else.

## 2. What is being tested, and what is not

**In scope:** `admet-core`, `admet-infer`, `admet-db`, `admet-api`, `admet-cli`,
the training pipeline's data handling and export path, the SvelteKit front end's
API contract, and the parity between the Python and Rust featurisers.

**Not in scope:** the scientific validity of the underlying ADMET assays;
RDKit's own correctness (it is the oracle, per assumption A-2 in the SRS); ONNX
Runtime's arithmetic beyond the parity tolerance; browser rendering fidelity
across engines beyond a smoke check.

One exclusion is worth defending because it looks like a gap: **RDKit is not
tested, it is trusted.** NFR-05 asserts our descriptors equal RDKit's exactly. If
RDKit were wrong, this project would be wrong in the same direction as the entire
field, which is the correct place to be.

## 3. Test levels

### 3.1 The pyramid

Table 23.1. Counts are **targets for the finished system**, chosen from what the
requirement set implies rather than from a ratio.

| Level | Prefix | Target | Actual | Runs where | Wall-clock budget |
|---|---|---|---|---|---|
| Unit | `TC-U-` | ~180 | → per increment | pre-commit (core only) + CI | < 5 s whole level |
| Integration | `TC-I-` | ~40 | → | CI with `postgres:16-alpine` | < 60 s |
| System / E2E | `TC-SYS-` | ~14 | → | CI, and by hand before each tag | < 3 min |
| Performance | `TC-P-` | ~8 | → | on demand; recorded in `results/` | minutes, not gated |
| Security | `TC-S-` | ~16 | → | CI, at the level each threat lives | included above |

The shape is deliberate and the reason is `admet-core`: because it has no I/O
([ADR-02](adr/0002-hexagonal-crate-split.md)), a unit test there is *cheap*, so
pushing coverage down the pyramid costs almost nothing. That is what makes the
pre-commit hook viable — it runs `cargo test -p admet-core` and nothing else,
finishing in about a second.

The two counts most likely to be missed, and why they are set where they are:

- **~180 unit tests is not padding.** FR-02 alone needs one per malformed-SMILES
  class (unclosed ring, unclosed branch, unknown element, bad charge, stray
  bracket, aromatic mismatch, empty, whitespace-only, …) and each must assert the
  *byte offset*, not merely that it failed. The 33 feature columns need
  per-block tests. Boundaries at 128/129 atoms and 0.45/0.30 Tanimoto need
  explicit pinning.
- **~14 system tests is small on purpose.** They are slow and brittle relative to
  their yield. One per use case plus the offline test (FR-24) plus a handful of
  error paths is the right budget; anything more belongs a level down.

### 3.2 Tooling

| Concern | Tool | Invoked by |
|---|---|---|
| Rust unit + integration | `cargo nextest` (profile `ci` emits JUnit) | `just test-rust` |
| Rust doc examples | `cargo test --doc` | CI `test` job |
| Property tests | `proptest` in `admet-core` | part of the unit level |
| Coverage | `cargo llvm-cov` | `just coverage`, CI |
| Python | `pytest` | `just test-py` |
| Benchmarks | `criterion` | `just bench` |
| HTTP load | `oha` (or `k6`) | `just bench` |
| Lint gates | `cargo fmt --check`, `cargo clippy -D warnings`, `ruff` | `just check`, pre-commit, CI |
| Dependency audit | `cargo audit` | `just audit`, CI |

### 3.3 Identifier scheme

Defined once, in [`01-srs.md`](01-srs.md) §4, and not restated here. Two
conventions that make the IDs usable rather than decorative:

- **The ID appears in a comment on the test itself**, so `git grep TC-U-090` lands
  on the code.
- **Numbering leaves gaps.** Inserting a test beside its neighbour must never
  renumber another, because the SRS and the matrix cite these.

---

## 4. Test data

Four corpora, each with a different job. Keeping them separate matters: a fixture
that serves two purposes gets edited for one of them and silently weakens the
other.

| Corpus | Location | Size | Purpose | May it change? |
|---|---|---|---|---|
| **Golden feature vectors** | `fixtures/parity/` | ~50 molecules | TR-03: pins the 33-dim featuriser across two languages to 1e-6 | **Only with a `schema_version` bump.** Editing it to make a test pass is the failure this fixture exists to prevent. |
| **End-to-end parity fixture** | `fixtures/parity/manifest.json` + `.f32` blobs | ~20 molecules | TR-04: full Python→Rust agreement to 1e-4 | Regenerated whenever the model changes; the tolerance never loosens |
| **Malformed-input corpus** | `crates/admet-core/tests/data/malformed.txt` | ~40 strings | FR-02, NFR-06: every rejection class, each with its expected byte offset | Append-only. A new crash found in the wild is added here first, fixed second |
| **Descriptor reference set** | `fixtures/descriptors/rdkit_reference.csv` | 500 molecules | NFR-05: exact equality against RDKit | Regenerated only on an RDKit upgrade, and the upgrade is then a reviewed change |

The `.f32` blobs are raw little-endian float32 and are marked `binary` in
[`.gitattributes`](../.gitattributes). A fixture quietly corrupted by CRLF
translation would fail the parity test for a reason nobody would think to look
for — which is exactly the class of bug that costs a day.

**Never** in a fixture: a real proprietary structure. The corpora are drawn from
public data, and that is a licence and confidentiality requirement, not a
convenience.

## 5. The tests that carry the most weight

Most tests confirm what is already believed. These few would each catch a defect
that is otherwise invisible until it has done damage, and they are named
individually so they cannot be quietly deferred.

| Test | Catches | Why it is invisible otherwise |
|---|---|---|
| `TC-U-090` featuriser golden vectors, 1e-6 | Python/Rust feature skew (R3) | Skew produces *plausible wrong predictions*, not errors. Nothing crashes, no log line appears, and the metrics look fine because training and serving each behave self-consistently. |
| `TC-I-050` end-to-end ONNX parity, 1e-4 | Any drift between the trained artefact and the served one | Covers the whole chain, including tensor packing order — the mistake a featuriser test cannot see. |
| `TC-U-003/004` 128/129 heavy-atom boundary | Truncation creeping in as a "fix" | A truncated molecule is a *different molecule*. Its prediction is wrong, not degraded, and it looks entirely normal. |
| `TC-U-057/058` Tanimoto 0.45 / 0.30 boundaries | An inequality flipped in a refactor | Off-by-one on a confidence threshold silently converts refusals into confident answers — the exact failure G5 exists to prevent. |
| `TC-U-070` geometric-mean triage, with the arithmetic mean asserted to **fail** | A "simplification" to the mean | The arithmetic mean lets eleven good endpoints hide one fatal one. Both produce a number in `[0,1]`, so no test that only checks the range would notice. |
| `TC-U-095` split determinism across two runs at one seed | R10, R4 | A non-deterministic split means every reported metric is unreproducible, and it is discovered at presentation time. |
| `TC-S-020` `Debug` output excludes the database password | Credential in the first log line | A derived `Debug` reintroduces this in one keystroke, and the leak is in logs before anyone reads them. |
| `TC-SYS-020` desktop UC-01 with the network interface **disabled** | A hidden runtime dependency | "No network calls in the code" is an inspection. Pulling the cable is a test. |
| `proptest` on the parser: arbitrary bytes never panic | NFR-06 | Hand-written cases test the malformed inputs one *imagines*. The generator finds the ones one does not. |

## 6. Coverage

NFR-04: **≥ 75% on `admet-core` and `admet-infer`.** Measured by
`cargo llvm-cov`, reported per crate, and enforced in CI once Increment 1 lands.

Those two crates and not the others, for a reason: they are where a defect is
*silent*. A broken route returns the wrong status code and someone notices within
a day; a broken feature column returns a number, and nobody notices at all.

Coverage is a floor, not a goal. Two rules keep it honest:

- **A line covered by a test with no assertion is not covered.** `nextest` will
  happily run a test that calls a function and checks nothing.
- **Chasing the last few percent by testing getters is worse than leaving them
  uncovered**, because it inflates the number that is supposed to be evidence.

Where coverage will be low by design: `main.rs` wiring, `Display` impls, and error
branches for conditions that cannot be produced without a fault injector. These
are listed in the coverage report's exclusions rather than tested for the metric's
sake.

---

## 7. Performance testing

A performance test produces a **number**, not a pass. Gating a build on a latency
threshold measured on shared CI hardware produces flaky failures and teaches
people to re-run the job, which is worse than not measuring.

| ID | Measures | Method | Lands in |
|---|---|---|---|
| `TC-P-001` | Single-prediction latency, warm | `oha`, 60 s, concurrency 1/4/16, p50/p95/p99 | `results/latency.json` |
| `TC-P-002` | Single-prediction latency, **cold** | Same, cache cleared, process just started | `results/latency.json` |
| `TC-P-003` | Micro-batch sweep | `criterion` over batch 1/2/4/8/16/32/64/128 | `results/batch-sweep.json` |
| `TC-P-004` | 10,000-row batch wall-clock | `just bench-cli N=10000`, container limited to **2 CPUs** | `results/throughput.json` |
| `TC-P-005` | Peak RSS during that batch | `docker stats` sampled through the run | `results/footprint.txt` |
| `TC-P-006` | Cold start to first `/readyz` 200 | Timed loop | `results/startup.txt` |
| `TC-P-007` | `intra_threads` 1 vs 2 vs 4 at fixed concurrency | Repeat `TC-P-001` per setting | `results/threads.json` |
| `TC-P-008` | `lto = "thin"` vs `"fat"` | Build both, repeat `TC-P-001`, record binary size too | `results/lto.json` |

Three conditions without which the numbers are not evidence:

- **Report cold as well as warm.** NFR-01's target says "warm cache", so quoting
  only the warm figure is technically compliant and practically misleading.
- **Constrain the CPU for `TC-P-004`.** NFR-02 says 2 vCPU. A figure from an
  unconstrained laptop does not test it.
- **Record the hardware, the commit, and the model version with every run.** A
  latency number without those cannot be compared to the next one.

## 8. Security testing

Each `TC-S-` maps to exactly one STRIDE row in
[`02-design.md`](02-design.md) §6.2. The mapping is one-to-one so that an
unmitigated threat is visibly untested rather than absent.

| ID | Threat | Assertion |
|---|---|---|
| `TC-S-001` | DoS via body size | A 21 MB body returns 413 and the process RSS does not grow by 21 MB |
| `TC-S-002` | DoS via slow request | A request exceeding 30 s returns 504 and the connection is released |
| `TC-S-003` | Tampering via oversized input | A 1,001-character SMILES is rejected **before** parsing |
| `TC-S-005` | DoS via row count | 50,001 rows rejected with the limit stated |
| `TC-S-010` … `TC-S-014` | Spoofing / EoP | Argon2id verification, cookie flags, no cross-account project read, no client-supplied user id trusted |
| `TC-S-020` | Info disclosure | `format!("{:?}", state)` contains no password |
| `TC-S-021` | Info disclosure | A database connection failure logs no URL |
| `TC-S-022` | Info disclosure | A 500 body carries no file path, SQL, or backtrace |

## 9. Entry and exit criteria per increment

A tag is the artefact that proves an increment happened (TR-10), so the exit
criteria are the conditions for cutting one.

| Increment | Entry | Exit — all of these, or no tag |
|---|---|---|
| 0 — Scaffold | Toolchain present | `cargo build --workspace` clean; `clippy -D warnings` clean; CI green; `just --list` works; tag `v0.0.1-scaffold` |
| 1 — Model core | Scaffold tagged; TDC data downloaded and profiled | Golden fixture committed; `TC-U-090` green (1e-6); ONNX exported at the opset TR-01 names; metrics for **both** split strategies in `results/`; 5 seeds with mean ± sd; tag `v0.1.0-model` |
| 2 — Inference service | `model.onnx` and `feature_schema.json` exist | `TC-I-050` end-to-end parity green (1e-4); FR-01–08, FR-10, FR-11 tested; NFR-05 exact-equality suite green; `TC-P-001` recorded; tag `v0.2.0-api` |
| 3 — Web workspace | API stable | UC-01 completable in a browser with no manual step; NFR-10 visually verified and screenshotted into `docs/evidence/`; FR-23 security tests green; tag `v0.3.0-web` |
| 4 — Batch + explainability | Web workspace usable | UC-02 on ≥1,000 rows end to end; `TC-P-004` recorded on 2 vCPU; attribution mapping tested (`TC-U-080`); export round-trip verified; tag `v0.4.0-batch` |
| 5 — Packaging | Everything above | Desktop UC-01 with the network **disabled** (`TC-SYS-020`); installer size recorded against NFR-12; clean-checkout run from documented steps only; tag `v0.5.0-desktop` |

The last exit criterion of Increment 5 is the one most often skipped and the one
most worth keeping: **a clean checkout, set up using only the written steps, with
no undocumented fix.** Every project works on the machine it was built on.

---

## 10. Defect log

### 10.1 How this log is used

A defect gets an ID when it is **found**, not when it is fixed. That ordering is
the whole value of the log: it records what went wrong and what it cost, which is
the only honest source for the "what I learned" section of a report.

- IDs are sequential and permanent: `DEF-01`, `DEF-02`, … Never reused.
- The fixing commit carries `Refs: DEF-nn`, so
  `git log --oneline --grep="DEF-"` is the defect history in order.
- Every defect that reached a passing test suite must answer **"which test would
  have caught this?"** — and that test is then written, in the same change.
- A defect found by a person rather than a test is more valuable than one found by
  CI, because it names a hole in the suite.

Severity, decided by consequence rather than by feeling:

| Severity | Meaning |
|---|---|
| **S1** | Produces a wrong number that looks right. Featuriser skew, truncation, a flipped threshold. |
| **S2** | Breaks a requirement outright. A route 500s, a batch loses rows. |
| **S3** | Wrong behaviour with an obvious symptom. Bad error message, wrong status code. |
| **S4** | Cosmetic or documentation. |

S1 outranks S2 deliberately. An outage is noticed in minutes; a plausible wrong
prediction is acted on.

### 10.2 The log

Fifteen rows were reserved because the manual's template expects roughly that many
across five increments. Empty rows are not padding — a log with three entries at
the end of fifteen weeks means defects were fixed without being recorded, and that
is a documentation failure rather than a quality achievement.

> **All fifteen were consumed before Increment 1 finished.** Not because the code
> was unusually bad, but because a scaffold had been written without a compiler,
> a linker, a package manager or a single executed test on the machine — and
> compiling it for the first time surfaced everything at once. Three were S1: a
> fixture that could not be loaded, a default value that violated its own
> invariant, and a triage score that did not disqualify a fatal endpoint. Two more
> were checks that had never run at all (a secret scanner failing on every input,
> a bench recipe that could not accept its own flags), which is the category worth
> fearing: a gate that has never fired is indistinguishable from one that works.
>
> The manual's estimate of "roughly fifteen across five increments" was not wrong
> about the rate of defect discovery. It assumed the code would be written and run
> in the same sitting.

| ID | Sev | Found in | Found by | Symptom | Root cause | Fix | Test added | Refs |
|---|---|---|---|---|---|---|---|---|
| DEF-01 | S2 | `fixtures/spike_tiny_gin.onnx` | Running `dump_parity_fixture.py` for the first time | `onnxruntime` refused the committed model: "External data path does not exist: `spike_tiny_gin.onnx.data`". The fixture CI was meant to prove the round-trip with could not be loaded at all. | `torch.onnx.export(dynamo=True)` defaults to `external_data=True`, writing weights to a sidecar file. Only the `.onnx` was copied into `fixtures/`. | `external_data=False` in the export (47 KB needs no sidecar), plus a `--publish` flag that copies the artefact and **asserts it loads standalone** before declaring success. | Publish path constructs an `InferenceSession` on the published copy — the check whose absence allowed this. | TR-01 |
| DEF-02 | S2 | `crates/admet-infer` | First-ever `cargo build` | The crate did not compile: 6 errors against `ort 2.0.0-rc.13`. | Written without a compiler present (`cargo` was not installed). rc.13 made `Error<R>` generic, turned `Session::inputs`/`outputs` into methods, and does not export `SessionInputValue`. | Added an `OrtBuilder` variant for `Error<SessionBuilder>`, switched to `(shape, Vec<f32>)` tensors, `try_extract_tensor`, and named feeds bound to graph order. **Dropped `ndarray`** — it was only a coupling to whichever version `ort` was built against. | `tests/onnx_parity.rs` ran for the first time and passes. | ADR-01, R6 |
| DEF-03 | S1 | `admet-core::graph` | `cargo test` | `MolGraph::default()` failed its own `validate()`: "nbr_offsets must have length n_atoms + 1". | `#[derive(Default)]` gives `nbr_offsets = vec![]`, but CSR needs the trailing sentinel even for zero atoms. Every graph built with `..Default::default()` inherited a malformed adjacency. | Hand-written `Default` with `nbr_offsets: vec![0]`. | `graph::tests::empty_graph_is_valid_and_empty` (existed; had never been run). | TR-02 |
| DEF-04 | S1 | `admet-core::features` | `cargo test` | `Hybridisation::Unknown` set the **Sp3d2** bit, not Sp3 as its own comment promised. | `set_one_hot` clamps with `value.min(width - 1)`; `Unknown` has discriminant 5 and the block is 5 wide, so the generic clamp landed on index 4 — octahedral, the rarest state in drug-like space — for every atom of undetermined hybridisation. | Explicit `match` mapping `Unknown → Sp3` before encoding. | `features::tests::out_of_range_values_clamp_into_edge_buckets`. | FR-03, TR-03 |
| DEF-05 | S1 | `admet-core::triage` | `cargo test` | A compound with one fatal endpoint scored **0.119**, not ~0. The module's stated load-bearing property did not hold. | The `d.max(1e-6)` log floor was doing double duty. With eleven perfect endpoints and one zero at weight 2, `exp(2·ln(1e-6)/13) = 0.119` — a likely hERG blocker ranked as a middling candidate. | A zero desirability is now **absorbing**, checked before the logs, which is what the geometric mean of a set containing zero actually is. The floor's only job is keeping `ln` finite for small-but-survivable values. | `triage::tests::one_disqualifying_value_sinks_the_geometric_mean`. | FR-11 |
| DEF-06 | S3 | `crates/admet-api` | First-ever `cargo build` | `cannot find module or crate sqlx` in `state.rs` and `main.rs`. | The API named `sqlx::PgPool` directly without declaring the dependency — and doing so would have breached the ADR-02 boundary that puts the driver behind `admet-db`. | `admet-db` re-exports the pool as `admet_db::Pool`; the API uses that and still has no `sqlx` dependency. | Compilation is the test: the boundary now fails to build if it is crossed. | ADR-02 |
| DEF-07 | S3 | `justfile` | `just check` | `cargo: command not found` from every recipe, while `cargo --version` worked in the same terminal. | `set shell := ["bash", "-uc"]` — and on a machine with WSL enabled, bare `bash` resolves to `C:\windows\system32\bash.exe`, the WSL launcher. Recipes were running in a different operating system. | `set windows-shell := ["C:/Program Files/Git/bin/bash.exe", "-uc"]`. `set shell` still governs Linux CI runners. | `just check` green end to end. | ADR-07 |
| DEF-08 | S4 | `crates/admet-api` | `cargo clippy -D warnings` | `TimeoutLayer::new` deprecated in `tower-http` 0.6.11; `panic` key in `[profile.bench]` warned on every build. | Upstream deprecation, and a manifest key Cargo documents as ignored for that profile. | `TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, ..)`; the ignored `panic` key removed and its intent kept as a comment. A permanent warning is how real warnings get missed. | `just check` is warning-free. | NFR-09 |
| DEF-09 | S2 | `.githooks/pre-commit` | First real `git commit` | The secret scan failed on **every** commit regardless of content, printing `grep: unknown option -- ---BEGIN [A-Z ]*PRIVATE KEY-----`. | The pattern list is passed as `grep -inE "$p"`, and the private-key pattern begins with `-----`, so grep parsed it as options. Two consequences: the scan never ran, and the private-key pattern had never matched anything in its life. | `grep -inE -e "$p"`. Separately, placeholder credentials (`:changeme@` and similar) are filtered by **value** so the local dev DSN in `config/default.toml` and the `justfile` does not trip the DSN rule — excluding those files instead would have blinded the scanner to a real password pasted into the same field. | Negative test run by hand: a staged file containing a real-shaped DSN, an `AKIA…` key and a `-----BEGIN RSA PRIVATE KEY-----` header now trips all three patterns; the clean tree passes. | NFR-11 |
| DEF-10 | S4 | `admet-core`, `admet-db`, `admet-api` | CI `Docs` job (`RUSTDOCFLAGS: -D warnings`) | `cargo doc` failed on three unresolved intra-doc links. | `[`RingTable::close`]` names a method that does not exist — it is `take`. Two `[`PgPool`]` links and one `[`PgPool`](sqlx::PgPool)` were left dangling by the DEF-06 rename to `admet_db::Pool`. | Links corrected to `RingTable::take`, `crate::Pool` and `admet_db::Pool`. Treated as a real defect, not a typo: these documents are graded, and a comment that links to a method which does not exist is wrong about the code it describes. | `just docs-check` added to `just check`, so rustdoc is now a local gate rather than a CI-only one. | NFR-10 |
| DEF-11 | S4 | `training/scripts/*.py` | CI `python` job (`ruff check`) | Two `EXE001` findings: "Shebang is present but file is not executable". | The two spike scripts carry `#!` lines without the git executable bit. **This lint cannot fire on Windows** — there is no executable bit to be missing — so no amount of local diligence would have caught it. | `git update-index --chmod=+x` on both, and on `scripts/verify-env.sh` and `.githooks/pre-commit`, which git executes directly and which would have broken on the first Linux clone. | `just lint-py` added to `just check`, running `ruff check` **and** `ruff format --check` as CI does. The deeper fix is that `just check` now runs CI's actual command set instead of a convenient subset — the local gate had been claiming a guarantee it did not provide. | NFR-10 |
| DEF-12 | S2 | `training/data/download_tdc.py` | First run of the downloader | Crashed before fetching a single byte: `AttributeError: module 'tdc' has no attribute '__version__'`. | PyTDC 1.1.15 does not define `__version__` at module level, and the downloader printed it. | Version read from installed distribution metadata via `importlib.metadata.version("PyTDC")`, falling back to `"unknown"`. Knowing which TDC produced a dataset is useful; it is not worth losing the dataset over. | Reproducing is the test: the script now completes and records the version in `_manifest.json`. | TR-07 |
| DEF-13 | S3 | `training/data/download_tdc.py` | Comparing download output with `docs/06-data-sources.md` | Five endpoints reported `DRIFT` against their published row counts — `ppbr` apparently 2,790 rows against a documented 1,797, which reads like a dataset revision that would invalidate every benchmark comparison. | Nothing had drifted. The published TDC figures are **unique-molecule** counts while the benchmark-group CSVs contain repeated SMILES: 993 duplicates in `ppbr`, 55 in `bbb`. The check compared raw rows. | The comparison is now made on unique SMILES, and duplicate counts are printed and recorded as `n_unique_smiles` in the manifest — duplicates being the information actually worth having, since they matter for Increment 1 dedup. | `training/scripts/check_row_count_drift.py`: for all twelve endpoints, `rows - duplicates` reproduces the published figure exactly or the raw count already matched. **12 explained, 0 unexplained.** Re-running the downloader reports zero DRIFT. | TR-07 |
| DEF-14 | S3 | `requirements-data.txt` | Following the file's own install instructions | `uv pip install -r requirements-data.txt` failed with `Failed to build tiledbsoma==1.11.4` … `not a CMake build directory`. On the retry the visible error was `ModuleNotFoundError: No module named 'pandas'`, pointing at entirely the wrong problem. | PyTDC 1.1.15 depends on `tiledbsoma`, a single-cell genomics package with no Windows wheel that tries to compile from source. The failed build aborts the whole transaction, so none of the other packages install either — hence the misleading pandas error. | PyTDC is installed separately with `--no-deps` and **removed from the requirements file**, so `-r` cannot re-resolve its tree. The file now lists the set `admet_group` genuinely needs, found by import-and-add, including a `setuptools<81` pin because `tdc/oracles.py` imports `pkg_resources` at import time and setuptools 84 removed it. `rdkit` is absent by design: the hard rule means this env does no chemistry, so PyTDC's `rdkit<2024.3.1` pin never has to be satisfied — the very conflict that justified splitting the environments. | The env was deleted and rebuilt using only the documented commands, then `admet_group` imported and all twelve endpoints downloaded. | TR-07, R3 |
| DEF-15 | S3 | `justfile` (`results`, `bench`) | Running `cargo bench` with a Criterion flag for the first time | `error: Unrecognized option: 'save-baseline'`, exit 101. `just results` — the recipe whose entire purpose is regenerating every number in the report — could never have worked. | `cargo bench -p admet-core` runs the **lib** target as a bench in addition to the Criterion target. The lib target uses the built-in libtest harness, which rejects Criterion's arguments. The plain form appears to work only because nothing had ever passed a Criterion flag through it. | Both recipes now name the bench target: `cargo bench -p admet-core --bench core`. | `just bench` and `cargo bench -p admet-core --bench core -- --save-baseline scaffold` both succeed; baselines captured in `docs/evidence/increment-0/benchmarks.md`. | NFR-01, NFR-02 |
| DEF-16 | S4 | `docs/04-traceability.md` §9 | First run of `scripts/check-traceability.py` | The gap summary claimed **61** distinct `TC-` IDs where the file contains **81** — and every category was low: 23 vs 33 U, 14 vs 19 I, 13 vs 15 SYS, 5 vs 6 P, 6 vs 8 S. | Two problems. The count was a hand count that reproduced under no method, and the metric was never defined: §8 abbreviates with ranges (`TC-U-001 … TC-U-009`), so a block of nine contributes two literal tokens while §4–§6 name individual IDs inside those ranges. §1 had claimed since August that the matrix was "machine-checked" and spelled the check out as four shell greps — which nobody had ever run. | The row now states what it counts (distinct literal `TC-` tokens), carries the corrected 81 with its per-category breakdown, and explains why that is *not* the number of planned tests. `scripts/check-traceability.py` implements the check §1 specified; `just trace` runs it and it is in `just check` and CI. | The script itself, verified in three directions: a stale count fails, an untraced `TC-` id in source fails, and a **renamed claim row fails** rather than silently skipping — that last one caught itself, when renaming this row dropped the verified count from 4 to 3 while the run still reported OK. | NFR-10 |
| DEF-17 | S3 | `docs/01-srs.md` §3.5 | `scripts/check-traceability.py` in CI | Two problems in one run. The check died with `FATAL cannot find requirements.md` and exit 2 in CI, and once pointed at the committed SRS instead it found **NFR-08 has no definition anywhere in the repository**. | `requirements.md` is gitignored by design — `.gitignore` carries `/*.md` so the root planning notes stay local — so a check that reads it can only run on one laptop. Repointing the register at the committed `docs/01-srs.md` then exposed the real gap: NFR-08 (statelessness) was cited in the matrix and present in the private register, but the SRS enumerates only the *quantitative* NFRs in §3.3 and defines the qualitative ones in §3.5 prose, where NFR-08 had simply never been written. A reader working from the repository alone could not discover what it required. | `docs/01-srs.md` §3.5 gains a Statelessness paragraph — no per-user session state, the only server state being the `(inchikey, model_version)` cache from ADR-04, and the two consequences that make it a requirement rather than a note: a second instance needs no sticky sessions, and a restart loses only warmth. The script now uses the SRS as the register and cross-checks `requirements.md` only when present. | The script, in both conditions: it passes with the private register present and with it hidden, and the local-vs-SRS divergence check is what surfaced NFR-08. | NFR-08, NFR-10 |
| DEF-18 | | | | | | | | |
| DEF-19 | | | | | | | | |
| DEF-20 | | | | | | | | |

Worked example of a completed row, so the level of detail is unambiguous:

> | DEF-07 | S1 | Increment 2 | `TC-I-050` in CI | Rust predictions differed from Python in the 3rd decimal for molecules with aromatic nitrogen | Python's featuriser set the aromaticity flag from RDKit's ring perception; the Rust lexer set it from lower-case symbols, which disagree for pyridine N-oxides | Rust now derives aromaticity from ring perception after parsing, matching RDKit | `TC-U-092`, plus three N-oxides appended to the malformed/edge corpus | `Refs: DEF-07, TR-03, R3` |

That row is fictional and labelled as such. It is here because "root cause" is the
column that gets written as "bug in featuriser" when nobody is watching, and that
sentence is worth nothing six weeks later.

## 11. Known gaps in this plan

Stated rather than discovered by an assessor:

- **No mutation testing.** Coverage counts executed lines, not assertions that
  would fail if the code were wrong. `cargo-mutants` on `admet-core` would be the
  strongest single addition to this plan and is out of scope for fifteen weeks.
- **No cross-browser matrix.** One engine, smoke-tested.
- **CI runs Linux only**, while development is on Windows — a deliberate gap with
  its reasoning in [ADR-07](adr/0007-native-windows-and-training-layout.md). The
  mitigation is that the desktop build in Increment 5 is built and run on Windows,
  so the platform is exercised before submission rather than assumed.
- **No load test above modest concurrency.** The target envelope is 2 vCPU; a
  1,000-connection test would measure the wrong thing.

---

## Appendix A — Revision history

| Version | Date | Change |
|---|---|---|
| 0.1 | 2026-08-27 | Strategy baselined. Pyramid targets set, four test corpora defined, high-value tests named, per-increment exit criteria written, defect log opened at DEF-01 … DEF-15. |
