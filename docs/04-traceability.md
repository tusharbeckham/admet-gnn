# 04 — Traceability Matrix

| | |
|---|---|
| **System** | ADMETriage |
| **Document** | Cross-reference listing: goal → requirement → design → code → test |
| **Status** | Seeded at the scaffold. `Code` and `Test` columns fill in per increment. |
| **Version** | 0.1 (scaffold) |
| **Traces** | [`requirements.md`](../requirements.md) · [`01-srs.md`](01-srs.md) · [`02-design.md`](02-design.md) · [`03-test-plan.md`](03-test-plan.md) |

## 1. What this document is for, and how to keep it true

A traceability matrix answers two questions that nothing else in the repository
answers:

- **Forward:** for this requirement, what code implements it and what test proves
  it? A row with an empty `Test` cell is an unverified claim.
- **Backward:** for this test, which requirement does it serve? A test that traces
  to nothing is either dead weight or evidence of an undocumented requirement.

### The maintenance rule

This matrix is **hand-maintained but machine-checked**. Writing it by hand is what
makes the gaps visible; checking it by script is what stops it rotting.

The check, once the tests exist, is two greps that must agree:

```bash
# Every requirement ID cited in the matrix must exist in the register.
grep -oE '(FR|TR|NFR|UC)-[0-9]+' docs/04-traceability.md | sort -u > /tmp/matrix.ids
grep -oE '(FR|TR|NFR|UC)-[0-9]+' requirements.md            | sort -u > /tmp/register.ids
diff /tmp/matrix.ids /tmp/register.ids

# Every test ID cited in the matrix must exist in the code.
grep -oE 'TC-[UISYSP]+-[0-9]+' docs/04-traceability.md | sort -u > /tmp/matrix.tc
git grep -hoE 'TC-[UISYSP]+-[0-9]+' -- crates training    | sort -u > /tmp/code.tc
comm -23 /tmp/matrix.tc /tmp/code.tc   # cited here, absent in code == a lie
comm -13 /tmp/matrix.tc /tmp/code.tc   # in code, uncited here == an untraced test
```

Both directions matter. The first catches a matrix that promises a test nobody
wrote; the second catches a test whose requirement was never written down.

**Legend.** `→` marks a cell that must be filled by the increment named in the
`Inc` column. It is a task, not a blank. `✅` marks something that exists in the
scaffold today.

---

## 2. Goals → requirements

Eight goals, and the requirements that discharge each. A goal whose requirements
all live in one increment is a goal that fails entirely if that increment slips —
which is why G8 is spread across the whole project and G1 is front-loaded.

| Goal | Discharged by | Increments | At risk from |
|---|---|---|---|
| G1 — twelve endpoints from SMILES alone | FR-01, FR-05, FR-08, TR-01, TR-02 | 1–2 | R1, R2 |
| G2 — single report under 300 ms p95 | NFR-01, TR-05, TR-06 | 2 | — |
| G3 — screen 10,000 without intervention | FR-14–FR-18, NFR-02, TR-08 | 4 | R7 |
| G4 — explain at atom level | FR-19, FR-20 | 4 | R9 |
| G5 — state when out of competence | FR-10, FR-11, FR-12, NFR-10 | 2–4 | R4 |
| G6 — local, offline, zero cost | FR-24, NFR-09, TR-02 | 2, 5 | — |
| G7 — web and desktop, one codebase | FR-24, NFR-12, and [ADR-02](adr/0002-hexagonal-crate-split.md) | 3, 5 | R8, R9 |
| G8 — documented defensibly | TR-10, TR-11, TR-12, NFR-04, NFR-07, this matrix | all | R9, R10 |

## 3. Use cases → requirements

| UC | Use case | Requirements | Inc | System test |
|---|---|---|---|---|
| UC-01 | Single molecule | FR-01–FR-11, FR-13 | 2–3 | → TC-SYS-001 |
| UC-02 | CSV batch | FR-14, FR-15, FR-16 | 4 | → TC-SYS-010 |
| UC-03 | Rank and triage | FR-12, FR-17, FR-18 | 4 | → TC-SYS-011 |
| UC-04 | Per-atom attribution | FR-19, FR-20 | 4 | → TC-SYS-012 |
| UC-05 | Compare to approved drugs | FR-21 | 4 | → TC-SYS-013 |
| UC-06 | Export CSV / PDF | FR-22 | 4 | → TC-SYS-014, TC-SYS-015 |
| UC-07 | Projects and saved sets | FR-23 | 3 | → TC-SYS-016 |
| UC-08 | Offline desktop | FR-24 | 5 | → TC-SYS-020 |

## 4. Functional requirements — forward trace

| FR | UC | Inc | Design | Code | Test |
|---|---|---|---|---|---|
| FR-01 | UC-01 | 2 | [§3.1.2](01-srs.md) | `admet-api/src/routes/predict.rs` ✅ stub | → TC-SYS-001, TC-I-001 |
| FR-02 | UC-01 | 2 | [02 §5.1](02-design.md) | `admet-core/src/smiles/{lexer,parser}.rs` ✅ stub | → TC-U-010 … TC-U-024, TC-I-004 |
| FR-03 | UC-01 | 2 | [02 §6.3](02-design.md) | `admet-core/src/lib.rs` (`MAX_SMILES_LEN`) ✅ | → TC-U-002, TC-S-003 |
| FR-04 | UC-01 | 2 | [02 §6.3](02-design.md) | `admet-core/src/graph.rs` (`MAX_HEAVY_ATOMS`) ✅ | → TC-U-003, TC-U-004 |
| FR-05 | UC-01 | 1 | [ADR-04](adr/0004-inchikey-identity-and-cache-key.md) | `admet-core/src/canonical.rs` ✅ stub | → TC-U-030, TC-U-031 |
| FR-06 | UC-01 | 2 | [02 §4.1](02-design.md) | `admet-core/src/features.rs` ✅ stub | → TC-U-040, TC-I-006 |
| FR-07 | UC-01 | 2 | [01 §3.2.2](01-srs.md) | `admet-api/src/routes/predict.rs` ✅ typed | → TC-U-045, TC-SYS-004 |
| FR-08 | UC-01 | 2 | [02 §4.1](02-design.md) | `admet-infer/src/lib.rs` (`N_ENDPOINTS`) ✅ | → TC-I-010 |
| FR-09 | UC-01 | 3 | `method.md` (thresholds) | → `admet-core/src/triage.rs` | → TC-U-050, TC-SYS-006 |
| FR-10 | UC-01 | 2 | [ADR-05](adr/0005-scaffold-split-not-random.md) | `admet-core/src/fingerprint.rs` ✅ stub | → TC-U-055, TC-U-056 |
| FR-11 | UC-01 | 2 | [01 §3.2.3](01-srs.md) | `admet-core/src/fingerprint.rs` ✅ thresholds | → TC-U-057, TC-U-058 |
| FR-12 | UC-03 | 4 | [02 §5.1](02-design.md) | `admet-core/src/triage.rs` ✅ stub; `admet-db/src/model.rs` nullable ✅ | → TC-U-060, TC-SYS-007 |
| FR-13 | UC-01 | 3 | [01 §3.1.1](01-srs.md) | → `web/src/lib/components/MoleculeCanvas.svelte` | → TC-SYS-008 |
| FR-14 | UC-02 | 4 | [02 Fig 7.6](02-design.md) | → `admet-api/src/routes/batch.rs` | → TC-I-020, TC-S-005 |
| FR-15 | UC-02 | 4 | [02 Fig 7.6](02-design.md) | → `admet-db/src/repository/batch.rs` ✅ stub | → TC-I-022 |
| FR-16 | UC-02 | 4 | [02 §4.2](02-design.md) | `admet-db/src/repository/mod.rs` (`CHECKPOINT_INTERVAL`) ✅ | → TC-I-023, TC-SYS-010 |
| FR-17 | UC-03 | 4 | [01 §3.2.4](01-srs.md) | `admet-core/src/triage.rs` ✅ stub | → TC-U-070, TC-U-071 |
| FR-18 | UC-03 | 4 | [02 §4.2](02-design.md) | `admet-db/src/repository/prediction.rs` ✅ stub | → TC-I-025 |
| FR-19 | UC-04 | 4 | [02 Fig 7.4](02-design.md) | → `admet-infer/src/lib.rs` (attribution head) | → TC-U-080, TC-I-030 |
| FR-20 | UC-04 | 4 | [01 §3.1.1](01-srs.md) | → `web/src/lib/components/MoleculeCanvas.svelte` | → TC-SYS-012 |
| FR-21 | UC-05 | 4 | [06-data-sources.md](06-data-sources.md) | → `admet-core/src/triage.rs` (reference set) | → TC-I-032 |
| FR-22 | UC-06 | 4 | [01 §5 Q-4](01-srs.md) | → `admet-api/src/routes/export.rs` | → TC-SYS-014, TC-SYS-015 |
| FR-23 | UC-07 | 3 | [02 §6.2](02-design.md) | → `admet-api/src/routes/auth.rs`; `admet-db/src/model.rs` (`User`) ✅ | → TC-S-010 … TC-S-014, TC-SYS-016 |
| FR-24 | UC-08 | 5 | [02 Fig 7.8](02-design.md) | → `desktop/` | → TC-SYS-020 |

Two rows carry more weight than their neighbours and are worth reading twice.
**FR-04** is the one a well-meaning change turns into truncation; **FR-12** is the
one a well-meaning UI turns into a zero. Both have tests named above whose job is
to make that change fail loudly.

---

## 5. Technical requirements — forward trace

| TR | Inc | ADR / design | Code | Test / gate |
|---|---|---|---|---|
| TR-01 | 1 | [ADR-03](adr/0003-dense-adjacency-over-sparse-scatter.md) | `training/scripts/spike_onnx_export.py` ✅; `training/models/` export path | → TC-U-100; spike passes ✅ |
| TR-02 | 2 | [ADR-01](adr/0001-rust-serving-onnx-boundary.md) | `admet-infer/src/lib.rs` ✅ | → TC-P-001; inspection (no Python in `crates/`) |
| TR-03 | 1 | [02 §4.1](02-design.md) | `admet-core/src/features.rs` ✅; `training/scripts/dump_parity_fixture.py` ✅ | → TC-U-090, TC-U-091 |
| TR-04 | 2 | [02 §4.1](02-design.md) | `admet-infer/tests/onnx_parity.rs` ✅ | → TC-I-050 |
| TR-05 | 2 | [ADR-04](adr/0004-inchikey-identity-and-cache-key.md) | `admet-db/src/repository/prediction.rs` ✅ stub; cache config ✅ | → TC-U-035, TC-I-052 |
| TR-06 | 2 | [02 §6.2](02-design.md) | `admet-api/src/main.rs` (layer order) ✅; `config/default.toml` ✅ | `admet-api/src/config.rs` test ✅; → TC-S-001, TC-S-002 |
| TR-07 | 2 | [02 §4.2](02-design.md) | `admet-db/src/lib.rs` ✅ | CI `test` job with `postgres:16-alpine` ✅ |
| TR-08 | 4 | [02 Fig 7.6](02-design.md) | → `admet-api/src/routes/batch.rs` | → TC-P-005, TC-I-021 |
| TR-09 | 2 | [02 §5.1](02-design.md) | `admet-api/src/error.rs` ✅ | → TC-I-005 |
| TR-10 | all | [`implementation.md`](../implementation.md) §4 | `git tag`; `/version` route ✅ | tag exists per increment |
| TR-11 | 0 | [02 §7](02-design.md) | `.github/workflows/ci.yml` ✅ | CI green ✅ (coverage + licence gates → Inc 1) |
| TR-12 | 1 | [ADR-05](adr/0005-scaffold-split-not-random.md) | `training/data/scaffold_split.py` ✅ | → TC-U-095 |

TR-11 is marked partially complete on purpose. Format, clippy, test and audit run
today; coverage and licence checks arrive with Increment 1, because a coverage gate
over an empty implementation reports a meaningless number.

## 6. Non-functional requirements — forward trace

| NFR | Inc | Target | Measured by | Evidence lands in |
|---|---|---|---|---|
| NFR-01 | 2 | p95 < 300 ms, p99 < 600 ms warm | → TC-P-001, TC-P-002 | `results/latency.json` |
| NFR-02 | 4 | 10,000 rows < 90 s on 2 vCPU | → TC-P-004 | `results/throughput.json` |
| NFR-03 | 1 | mean AUROC ≥ 0.80 scaffold-held-out | `just results`, 5 seeds | `results/metrics.json` |
| NFR-04 | 1 | ≥ 75% on `admet-core`, `admet-infer` | `cargo llvm-cov` in CI | CI artefact per run |
| NFR-05 | 2 | descriptors **exactly** equal RDKit | → TC-U-040, TC-I-006 | test result, not a number |
| NFR-06 | 2 | never panics on malformed input | → `proptest` in `admet-core` | test result |
| NFR-07 | 1 | seed + revision reproduce split and metrics | → TC-U-095 | two runs diffed in `results/` |
| NFR-08 | 2 | `/predict` holds no session state | inspection + TC-I-001 | design review |
| NFR-09 | 2 | no paid API, runs offline | inspection of dependencies | `cargo tree` in the report |
| NFR-10 | 3 | UI distinguishes computed from predicted | manual, screenshotted | `docs/evidence/` |
| NFR-11 | 5 | cold start < 10 s | → TC-P-006 | `results/startup.txt` |
| NFR-12 | 5 | < 512 MB RSS; installer < 15 MB | → TC-P-005, `ls -l` on the bundle | `results/footprint.txt` |

**NFR-05, NFR-06 and NFR-08 have no number**, and that is correct rather than an
oversight: they are satisfied or not, so their evidence is a green test or a
recorded inspection, never a figure. Putting a percentage next to "matches RDKit
exactly" would weaken it.

**NFR-10's evidence is a screenshot**, which is the weakest form of evidence in
this document. It is accepted because the requirement is about visual distinction,
and an assertion on a CSS class would prove the class is applied rather than that a
human can tell the difference. The screenshot is dated and committed so at least
it is falsifiable.

## 7. Risks → mitigation → where it lives

| Risk | Mitigation | Implemented in | Verified by |
|---|---|---|---|
| R1 — ONNX export fails | Dense adjacency by design; spike **before** any Rust | `training/scripts/spike_onnx_export.py` ✅, [ADR-03](adr/0003-dense-adjacency-over-sparse-scatter.md) | spike passes at the opset TR-01 names |
| R2 — small endpoints do not learn | Shared multi-task trunk; per-endpoint honest reporting | `method.md`, `training/models/` | `results/metrics.json` per endpoint |
| R3 — featuriser skew | Golden fixture, 1e-6, generated from Rust and asserted by Python | `admet-core/src/features.rs` ✅, `fixtures/parity/` | → TC-U-090 |
| R4 — split implemented wrongly | One implementation, unit-tested against a hand-built example | `training/data/scaffold_split.py` ✅ (self-test present) | → TC-U-095 |
| R5 — scope creep to 22 endpoints | Endpoint list frozen; additions need an ADR | [ADR-06](adr/0006-tdc-over-moleculenet.md), `ENDPOINTS` dict ✅ | review |
| R6 — `ort` API churn | Exact version pin, wrapped behind one type | `Cargo.toml` ✅, `admet-infer/src/lib.rs` ✅ | `cargo build` in CI |
| R7 — batch OOM | Bounded channel; hard 50,000-row cap | → `admet-api/src/routes/batch.rs` | → TC-P-005, TC-S-005 |
| R8 — Tauri breaks on an untested platform | Windows is the development host from week one | [ADR-07](adr/0007-native-windows-and-training-layout.md) | → TC-SYS-020 |
| R9 — time runs out before Increment 5 | Increments ordered by defensibility; 1–3 alone present as complete | [`implementation.md`](../implementation.md) §4 | tags exist per increment |
| R10 — metrics not reproducible | `just results` regenerates; never hand-copied | [`justfile`](../justfile) ✅ | → TC-U-095 + two diffed runs |

R1's mitigation is in the past tense on purpose. The spike ran **before** any Rust
was written, which is the only ordering in which that mitigation has value — a
spike run after the architecture is committed is a formality.

---

## 8. Backward trace — test → requirement

The forward direction above is the one people write. This one is the one that finds
mistakes, because it asks the harder question: **why does this test exist?** A test
that traces to nothing is either dead weight or, more interestingly, evidence of a
requirement nobody wrote down.

Tests are grouped by block rather than listed one by one. There will be roughly 180
unit tests and listing each would make this table unreadable without making it more
true — the block is the unit of accountability, and the ID gaps inside each block
are deliberate (see [`03-test-plan.md`](03-test-plan.md) §3.3).

| Block | Level | Serves | Owning module | Inc |
|---|---|---|---|---|
| `TC-U-001` … `TC-U-009` | Unit | FR-03, NFR-06 | `admet-core/src/lib.rs`, `graph.rs` | 2 |
| `TC-U-010` … `TC-U-029` | Unit | FR-02, NFR-06 | `admet-core/src/smiles/` | 2 |
| `TC-U-030` … `TC-U-039` | Unit | FR-05, TR-05 | `admet-core/src/canonical.rs` | 2 |
| `TC-U-040` … `TC-U-049` | Unit | FR-06, FR-07, NFR-05 | `admet-core/src/features.rs` | 2 |
| `TC-U-050` … `TC-U-059` | Unit | FR-09, FR-10, FR-11 | `admet-core/src/fingerprint.rs` | 2–3 |
| `TC-U-060` … `TC-U-079` | Unit | FR-12, FR-17, FR-18 | `admet-core/src/triage.rs` | 4 |
| `TC-U-080` … `TC-U-089` | Unit | FR-19 | `admet-infer/src/lib.rs` | 4 |
| `TC-U-090` … `TC-U-099` | Unit | TR-03, TR-12, NFR-07 | `features.rs`, `scaffold_split.py` | 1 |
| `TC-U-100` … `TC-U-109` | Unit | TR-01 | `training/models/` export path | 1 |
| `TC-I-001` … `TC-I-009` | Integration | FR-01, NFR-08, TR-09 | `admet-api/src/routes/` | 2 |
| `TC-I-010` … `TC-I-019` | Integration | FR-08 | `admet-api` ↔ `admet-infer` | 2 |
| `TC-I-020` … `TC-I-029` | Integration | FR-14–FR-16, FR-18, TR-08 | `admet-api`, `admet-db` | 4 |
| `TC-I-030` … `TC-I-039` | Integration | FR-19, FR-21 | `admet-infer`, `admet-core` | 4 |
| `TC-I-050` … `TC-I-059` | Integration | TR-04, TR-05 | `admet-infer/tests/`, `admet-db` | 2 |
| `TC-SYS-001` … `TC-SYS-009` | System | UC-01 | whole stack | 2–3 |
| `TC-SYS-010` … `TC-SYS-019` | System | UC-02 … UC-07 | whole stack | 4 |
| `TC-SYS-020` … | System | UC-08, FR-24, NFR-11 | `desktop/` | 5 |
| `TC-P-001` … `TC-P-008` | Performance | NFR-01, NFR-02, NFR-11, NFR-12 | measurement, not assertion | 2–5 |
| `TC-S-001` … `TC-S-005` | Security | TR-06, FR-03, FR-14 | `admet-api` layers | 2 |
| `TC-S-010` … `TC-S-014` | Security | FR-23 | `admet-api/src/routes/auth.rs` | 3 |
| `TC-S-020` … `TC-S-022` | Security | NFR-06, TR-09 | `admet-db`, `admet-api/src/error.rs` | 2 |

Two blocks trace to something other than an `FR`, and that is the interesting
result of doing this direction at all:

- **`TC-U-090` … `TC-U-099` serve `TR-03` and `NFR-07`, not a functional
  requirement.** Nothing a user asks for is "the two featurisers agree". It is a
  property of the build, and it is the single most valuable test in the suite (R3).
  A backward trace that only permitted `FR` targets would have declared it orphaned
  and someone would have deleted it.
- **`TC-S-020` … `TC-S-022` serve `NFR-06` and `TR-09`.** They assert what is
  *absent* from an output — no password, no URL, no backtrace. Absence has no
  natural place in a functional requirement, which is exactly why it goes untested
  in most projects.

## 9. Gap summary — the state of the matrix today

Counted by the scripts in §1 rather than by reading, on 2026-08-27 at the scaffold
tag. The point of putting real counts here is that they are falsifiable: re-run the
greps and they either reproduce or this section is stale.

| Measure | Count | Notes |
|---|---|---|
| Requirement IDs in the register | 56 | FR 24 + TR 12 + NFR 12 + UC 8 |
| …cited in this matrix | **56** | `diff` against the register is empty ✅ |
| FR/TR rows whose `Code` cell exists today | **27** of 36 | 16 FR + 11 TR, stub or real |
| FR/TR rows still `→` in `Code` | 9 of 36 | 8 FR + 1 TR: Increments 3–5, `web/` and batch |
| Distinct `TC-` IDs cited here | 61 | 23 U, 14 I, 13 SYS, 5 P, 6 S |
| …that exist in `crates/` or `training/` | **0** | every `Test` cell is a `→` |
| Test-plan target for the finished suite | ~258 | [`03-test-plan.md`](03-test-plan.md) §3.1 |

The row that matters is the sixth. **Not one test named in this document exists
yet**, and the honest reading of that is: *this matrix currently traces requirements
to intentions.* It becomes evidence at Increment 1, when `TC-U-090` lands and the
first `→` in the `Test` column turns into an ID that `git grep` can find.

That is stated here rather than left to be noticed because the failure mode of a
traceability matrix is looking complete. A matrix full of `→` is honest. A matrix
where `→` has been quietly replaced by a plausible-looking test name that nobody
wrote is worse than no matrix at all, and it is one careless afternoon away.

**The 61 cited IDs are a commitment, not a wish list.** Each one appears in a `Test`
cell above and in [`03-test-plan.md`](03-test-plan.md) or
[`01-srs.md`](01-srs.md) §3.2. When the code is written, the ID goes in a comment on
the test — that is what closes the loop and makes `comm -23` in §1 return nothing.

### Known holes, named rather than discovered

- **FR-09's risk-band calibration has no design document**, only a pointer to
  `method.md`. It is open question Q-3 in [`01-srs.md`](01-srs.md) §5, and until it
  closes, `TC-U-050` cannot be written because there is nothing to assert against.
- **FR-21's reference set of approved drugs is unsourced.** Q-5. The row traces to
  `triage.rs` and `06-data-sources.md`, but the file it needs does not exist.
- **TR-01's opset is contradicted by its own history** — the requirement and the
  spike say 17, commit `fad0660` says 18. Q-1. One of the three is wrong and the
  matrix cannot be trusted on that row until it is settled.
- **NFR-10 traces to a screenshot**, discussed in §6. It is the weakest cell in the
  document and is left visible rather than dressed up.

Four holes in fifty-six rows is a reasonable position at a scaffold tag. Zero would
mean the register had been written to match the code.

---

## 10. How to use this document during the build

Three habits, in the order they come up:

1. **Writing code?** Find the requirement's row first. If the `Code` cell names a
   path that is not where you are about to write, one of the two is wrong — and
   deciding which takes thirty seconds now against an hour in week twelve.
2. **Writing a test?** Put the `TC-` ID in a comment on it, then replace the `→` in
   this file in the same commit. The `Refs:` trailer
   ([`05-git-conventions.md`](05-git-conventions.md)) makes that pairing greppable.
3. **Finished an increment?** Run both check blocks from §1 before cutting the tag.
   `comm -23` returning a line means this document claims a test that does not
   exist, which is the specific lie the exit criteria in
   [`03-test-plan.md`](03-test-plan.md) §9 exist to prevent.

The one habit that keeps this document alive is the second. Everything else here is
recoverable by re-reading the code; a test whose requirement was never recorded is
not.

## Appendix A — Revision history

| Version | Date | Change |
|---|---|---|
| 0.1 | 2026-08-27 | Seeded at the scaffold tag. All 56 register IDs traced forward; 61 test IDs committed to; backward trace by block; risks R1–R10 mapped to their mitigations; gap counts measured by the §1 scripts rather than estimated. |
