# START HERE

| | |
|---|---|
| **System** | ADMETriage — explainable ADMET screening from SMILES |
| **Document** | The fifteen weeks, in order, with the exact command for each |
| **Status** | Written at the scaffold tag. Weeks 1–3 are partly done; week 4 onward is yours. |
| **Read with** | [`implementation.md`](implementation.md) §3 · [`docs/journal/README.md`](docs/journal/README.md) §3 |

Everything else in this repository describes *what* the system is. This document is
the only one that says **what to do on Monday.**

---

## 0. How to read this

Each week below has the same five parts, and nothing else:

| Part | Meaning |
|---|---|
| **Read** | manual chapters and repository documents to read *before* typing |
| **Touch** | the files that change this week — no others |
| **Run** | copy-pasteable commands for native Windows + Git Bash |
| **Done when** | exit criteria. All boxes ticked or the week is not finished |
| **Feeds** | which graded artefact this week's output ends up inside |

Two habits make the difference between this working and not:

1. **Write the journal entry Friday**, from [`WEEK-TEMPLATE.md`](docs/journal/WEEK-TEMPLATE.md).
   §3 of that template is what keeps [`04-traceability.md`](docs/04-traceability.md)
   true, and it takes two minutes on Friday versus an hour in November.
2. **Clear one `→` in the traceability matrix per feature.** When a test starts
   existing, put its `TC-` id in a code comment and delete the arrow in the same
   commit. Sixty-one arrows go away one at a time or they do not go away.

## 1. Where the project actually stands today

Read this table before believing any other document in the repository.

| Layer | State | Evidence |
|---|---|---|
| Planning documents | Complete — SRS, design, test plan, traceability, 7 ADRs | [`docs/`](docs/) |
| Python environment | Working — 3.12.10, torch 2.13.0+cpu, onnx 1.22, rdkit 2026.3.5 | `just verify` |
| ONNX spike | **Green** — dense GIN exports and round-trips | [`spike_onnx_export.py`](training/scripts/spike_onnx_export.py) |
| Rust — 5 crates, ~30 modules | Stubs with typed signatures. **Never compiled** — `cargo` is not installed | `crates/` |
| Data | TDC downloader written; **nothing downloaded yet**. `data/` still holds the old MoleculeNet prototype | [`legacy_moleculenet/`](training/legacy_moleculenet/README.md) |
| Tests | **Zero** of the 61 planned `TC-` ids exist | [`04-traceability.md`](docs/04-traceability.md) §9 |
| `web/`, `desktop/`, `migrations/` | README only, by design — Increments 3, 5, 2 | |

So the honest one-line status is: **the thinking is done and the typing has not
started.** That is a good position, but only if you do not mistake it for progress.

## 2. Do this first — it blocks everything

Nothing below week 1 can be verified until the Rust toolchain exists. Five tools are
missing: `rustc`/`cargo`, `just`, `pnpm`, `typst`, and the MSVC linker.

```bash
bash scripts/verify-env.sh
```

Red rows are the script working. Install what it names, from
[`docs/00-machine-setup.md`](docs/00-machine-setup.md) — in this order, because each
depends on the previous:

```bash
winget install --id Microsoft.VisualStudio.2022.BuildTools
```

```bash
winget install --id Rustlang.Rustup
```

```bash
cargo install just sqlx-cli cargo-nextest cargo-llvm-cov cargo-audit typst-cli
```

```bash
corepack enable pnpm
```

Reopen Git Bash after the first two — a running shell keeps the old `PATH`, which is
[troubleshooting §1](docs/reference/troubleshooting.md). Then:

```bash
bash scripts/verify-env.sh && cargo build --workspace
```

**The first `cargo build` is a real event.** Around 5,500 lines of Rust have never seen
a compiler. Expect errors, and expect them to be boring ones — a missing `mod`
declaration, an unused import that `-D warnings` rejects. Fix them in one commit typed
`chore(build): first compile of the scaffold` and note the count in the week-1 journal
entry, because "the scaffold compiled first try" and "the scaffold needed 40 fixes" are
different facts about the scaffold and only one of them is likely.

While Windows Defender is scanning every artefact `rustc` writes, builds are several
times slower than they need to be. Fix it once, in an administrator PowerShell:

```powershell
Add-MpPreference -ExclusionPath "C:\projects\Phore\target"
```

## 3. Three rules that hold for all fifteen weeks

**Rule 1 — a number is a target until a script produces it.** Every figure in `docs/`
(p95 < 300 ms, mean AUROC ≥ 0.80, 10,000 molecules < 90 s) is a design goal. Measured
values live in `results/`, are written by `just results`, and **a missed target is
recorded as a miss.** Editing a target down to meet a result is the one thing in this
project that would be dishonest rather than merely wrong; a documented miss with an
explanation reads as competence.

**Rule 2 — the featuriser exists twice, and skew is silent.** Python featurises for
training, Rust featurises for serving, and if they disagree the system returns
*plausible wrong answers* rather than crashing. That is risk R3, and the only defence is
the parity fixture. The direction matters and is not symmetric:

> `feature_schema.json` is generated **from Rust** (`admet_core::features::schema_json()`)
> and asserted **by Python**. The Rust featuriser is the contract because it is the one
> on the request path.

**Rule 3 — build vertically, one endpoint at a time.** From
[`implementation.md`](implementation.md) §7: get **`bbb` / `BBB_Martins`** (1,975 rows,
clean signal) working through *every* layer — data → clean → split → features → model →
ONNX → Rust → API → UI — before widening to twelve.

The failure mode this avoids is twelve half-trained endpoints, a half-written API, and no
working page, with three plausible causes for every bug and no way to bisect them. When
the vertical slice runs, widening to twelve endpoints is a loop, not a project.

## 4. The tags, and what each one promises

A tag is a claim that something works. Do not move one until it does.

| Tag | Week | The claim |
|---|---|---|
| `v0.0.1-scaffold` | 1 | Five crates compile, zero clippy warnings, CI green |
| `v0.1.0-model` | 6 | `model.onnx` loads in `onnxruntime`; per-endpoint metrics on the scaffold split exist for both models; golden fixture committed |
| `v0.2.0-api` | 8 | `POST /predict` returns a full report; end-to-end parity to 1e-4 verified in CI |
| `v0.3.0-web` | 10 | A chemist completes UC-01 in a browser without touching `curl` |
| `v0.4.0-batch` | 12 | A 1,000-molecule CSV screens end to end and returns a ranked table |
| `v0.5.0-desktop` | 13 | Commit to deployed in under 10 minutes, unattended |
| `v1.0.0` | 15 | Traceability matrix closed; report submitted |

```bash
git tag -a v0.0.1-scaffold -m "Scaffold: five crates build clean, CI green" && git push --follow-tags
```

Before **every** tag, run both self-checks from
[`04-traceability.md`](docs/04-traceability.md) §1. A line of output from either means a
document is claiming something untrue.

---

# The fifteen weeks

Chapter numbers are **manual** chapters
([index](docs/reference/build-manual-index.md)). `Fig 7.x` means a figure in
[`02-design.md`](docs/02-design.md), which is a different numbering system.

## Week 1 — toolchain, spike, scaffold, CI · Increment 0 · tag `v0.0.1-scaffold`

Mostly done. What remains is the part that needs a compiler.

**Read.** Manual ch. 2, 4, 5. Then [`00-machine-setup.md`](docs/00-machine-setup.md) and
[`05-git-conventions.md`](docs/05-git-conventions.md).

**Touch.** Nothing new. Fix whatever `cargo build` reports.

**Run.**

```bash
just verify && just build && just lint
```

```bash
just spike
```

```bash
just hooks && git commit --allow-empty -m "chore: verify pre-commit hook fires"
```

**Done when.**

- [ ] `scripts/verify-env.sh` exits 0 — every tool present
- [ ] `cargo build --workspace` succeeds and `cargo clippy --workspace --all-targets -- -D warnings` is silent
- [ ] `just spike` passes at batch sizes 1 / 7 / 64
- [ ] CI is green on a pushed branch (the `web` job is deliberately commented out until Increment 3)
- [ ] The pre-commit hook fires and can be seen to fail on purpose
- [ ] `docs/journal/week-01.md` exists
- [ ] `v0.0.1-scaffold` pushed

**Feeds.** Report method chapter (process model, toolchain);
[`03-test-plan.md`](docs/03-test-plan.md) §9 CI evidence.

**Resolve this week — open question Q-1.** TR-01 says opset 17,
[`spike_onnx_export.py`](training/scripts/spike_onnx_export.py) sets `OPSET = 17`, and
commit `fad0660` claims "verified ONNX export round-trip at opset 18". One of the three is
wrong, and the cost of finding out in week 8 instead is a day of confusion in the wrong
language.

```bash
just spike | grep -i opset
```

```bash
git show --stat fad0660 && grep -rn "opset" requirements.md training/scripts/spike_onnx_export.py
```

The spike exports into a temporary directory and deletes it, so there is no artefact to
inspect yet; from week 6, when `models/model.onnx` exists, the authoritative check is the
`onnx.load` one-liner in
[troubleshooting §12](docs/reference/troubleshooting.md). Whichever way it resolves, edit
the loser and record the resolution in [`01-srs.md`](docs/01-srs.md) §5.

## Week 2 — scaffold finished, requirements analysis begins · Increment 0

**Read.** Manual ch. 3 and 18.2 step 1. Then [`06-data-sources.md`](docs/06-data-sources.md)
and [ADR-06](docs/adr/0006-tdc-over-moleculenet.md).

**Touch.** `.venv-tdc`, `data/raw/tdc/`, `docs/01-srs.md`.

**Run.** Create the second environment — PyTDC pins `rdkit<2024.3.1` and this project
runs 2026.3.5, so they cannot share a venv (risk R3):

```bash
just setup-tdc
```

```bash
just data-download
```

```bash
just data-profile
```

`data-profile` is the step people skip, and it is the one that tells you an endpoint is
log-transformed, or that 3 % of a set exceeds the 128-atom cap, *before* you spend a week
training against it. Paste its output into the week-2 journal entry — it is the
provenance for the dataset table in the report.

**Done when.**

- [ ] `.venv` and `.venv-tdc` report **different** RDKit versions
- [ ] All twelve endpoints downloaded as raw CSV, with the PyTDC version recorded
- [ ] `profile.py` output captured: row counts, unparseable count, heavy-atom mean/max, fraction over 128, target mean/std
- [ ] The old MoleculeNet `data/raw` + `data/processed` moved aside or deleted, and [`MIGRATION.md`](MIGRATION.md) says why
- [ ] `docs/journal/week-02.md`

**Feeds.** Report dataset table; [`06-data-sources.md`](docs/06-data-sources.md) licence
obligations; the row-count column of the endpoint table everywhere it appears.

## Week 3 — requirements analysis · Increment 0

**Read.** Manual ch. 28 for the cross-reference format.

**Touch.** [`docs/01-srs.md`](docs/01-srs.md), `docs/diagrams/` (Figs 7.1–7.2).

**Run.** No commands. Drawing week. Follow the tool assignment and the nine consistency
rules in [`diagrams/README.md`](docs/diagrams/README.md) — in particular **colour means
ownership, never importance.**

**Done when.**

- [ ] Every FR/TR/NFR/UC in [`requirements.md`](requirements.md) has a numbered section in the SRS
- [ ] Use-case diagram (UC-01…UC-08) and context DFD exported as SVG **and** 300 DPI PNG
- [ ] Both traceability self-checks print nothing
- [ ] `docs/journal/week-03.md`

**Feeds.** Report requirements chapter; [`04-traceability.md`](docs/04-traceability.md) §3–§6.

## Week 4 — design, and Increment 1 starts · Increment 1

**Read.** Manual ch. 11, 12 (ER model, normalisation) and 27 (diagrams). Then
[`02-design.md`](docs/02-design.md) and [`crates/admet-db/src/model.rs`](crates/admet-db/src/model.rs),
whose row types are the ER model already written down in Rust.

**Touch.** `docs/02-design.md`, `docs/diagrams/` (Figs 7.3–7.8), `docs/adr/`.

**Run.**

```bash
just data-prepare
```

That is `clean.py` then `scaffold_split.py`, both on the **one** pinned RDKit. The split
has a self-test at the bottom of the file — benzene and toluene must land in the same
fold — and it is worth running before trusting the split:

```bash
.venv/Scripts/python.exe training/data/scaffold_split.py
```

**Done when.**

- [ ] ER model, class diagram, architecture diagram, DFD L1/L2 exported
- [ ] Any decision made this week that is not already ADR-01…ADR-07 has its own ADR
- [ ] `bbb` cleaned, deduplicated by InChIKey, molecules over 128 heavy atoms rejected with the count recorded
- [ ] Scaffold split written to disk with a fixed seed, and the same seed reproduces it
- [ ] `docs/journal/week-04.md`

**Feeds.** Report design chapter; every diagram in the register.

## Week 5 — features, dense GIN, first training runs · Increment 1

**Read.** Manual ch. 10 (the 33-feature contract) and 16–17 (model architecture).
Then [ADR-03](docs/adr/0003-dense-adjacency-over-sparse-scatter.md).

**Touch.** New Python: `training/features/atoms.py`, `features/dense.py`,
`models/dense_gin.py`, `models/mlp.py`, `train.py`.

**Run.** `bbb` only. Twelve endpoints is next week's problem.

```bash
just train --endpoint bbb --seed 1
```

**Done when.**

- [ ] The 33-dim vector is implemented **once** in Python, with the field order documented
- [ ] `x[B,128,33]`, `adj[B,128,128]` symmetric-normalised, `mask[B,128]` — shapes asserted in a test
- [ ] The fingerprint MLP baseline trains, because a GNN that cannot beat it is not worth reporting
- [ ] `bbb` trains, early-stops, and writes a best checkpoint
- [ ] Seeds 1–5 give the same result twice for the same seed (TR-12)
- [ ] `docs/journal/week-05.md`

**Feeds.** Report implementation chapter; `results/` first numbers.

**Watch for R2.** Row counts span 578 to 13,130. If a small endpoint will not learn, the
answer is the shared multi-task trunk with a masked loss, not a per-endpoint model — and
the decision is cheaper this week than in week 6.

## Week 6 — metrics, golden fixture, Increment 2 starts · tag `v0.1.0-model`

**Read.** Manual ch. 23 (test strategy), and re-read
[`03-test-plan.md`](docs/03-test-plan.md) §3.1 before writing any test, so the ids match.

**Touch.** `training/export.py`, `training/scripts/dump_parity_fixture.py`,
`fixtures/`, `results/metrics.json`, `results/model_card.md`.

**Run.** Widen to twelve endpoints, then export and freeze the fixture:

```bash
just train
```

```bash
just export
```

```bash
just parity-fixture && just schema
```

```bash
git check-attr binary -- fixtures/parity/aspirin.f32
```

That last line must print `binary: set`. If it does not, Windows will translate `0x0A`
bytes inside your float32 blobs and the parity test will fail with absurd values that
look exactly like a chemistry bug — [troubleshooting §5](docs/reference/troubleshooting.md),
described there as the single most expensive bug on this platform.

**Done when.**

- [ ] `models/model.onnx` exists, loads in `onnxruntime`, and only the batch axis is dynamic
- [ ] Per-endpoint metrics for **both** models on the scaffold split **and** the random split, in `results/metrics.json`
- [ ] `results/model_card.md` written, including the endpoints that miss NFR-03's 0.80
- [ ] `fixtures/parity/` committed with a manifest, 200 molecules
- [ ] `models/feature_schema.json` emitted **by Rust**, asserted by a Python test
- [ ] `docs/journal/week-06.md` and `v0.1.0-model` pushed

**Feeds.** Report evaluation chapter — this is the week that produces its central table.

**The number that matters.** Report the scaffold-split score as the headline and the
random-split score beside it. The gap between them is not an embarrassment; it is the
evidence that the split was done correctly, and a project reporting only the random
number is reporting a number inflated by 10–20 points
([ADR-05](docs/adr/0005-scaffold-split-not-random.md)).

## Week 7 — `admet-core`: parser, graph, features, fingerprint · Increment 2

**Read.** Manual ch. 6 (SoA/CSR), 7 with 7.1–7.3 (lexer, LL(1) parser, ring perception
via union-find), 8 (canonicalisation, InChIKey), 9 (scaffolds), 10, 14, 15.

**Touch.** [`crates/admet-core/src/`](crates/admet-core/src/) — the stubs are already
there with the right signatures. Fill bodies; do not restructure.

**Run.** This is the fastest inner loop in the project, about a second:

```bash
just test-core
```

```bash
cargo test -p admet-core -- --nocapture parser
```

**Done when.**

- [ ] Parse errors carry a **byte offset** into the input (FR-02) — `TC-U-010…029`
- [ ] `admet-core` panics nowhere; every fallible path returns `Result` (NFR-06)
- [ ] InChIKey is exactly 27 characters with hyphens at positions 15 and 26 (FR-05)
- [ ] Descriptors match RDKit exactly, not approximately (NFR-05) — `TC-U-040…049`
- [ ] Lipinski and Veber reported **per rule**, not as a violation count (FR-07)
- [ ] The 33-dim features match `fixtures/parity/` to 1e-6 (TR-03) — `TC-U-090…099`
- [ ] `admet-core`'s only dependency is still `thiserror` ([ADR-02](docs/adr/0002-hexagonal-crate-split.md))
- [ ] `docs/journal/week-07.md`

**Feeds.** Report implementation chapter; the unit-test row of the pyramid in
[`03-test-plan.md`](docs/03-test-plan.md) §3.1.

**If parity fails**, the bug is in Rust until proven otherwise — Python's featuriser was
what trained the model, so Python defines the target even though Rust owns the schema.
Diff one molecule field by field rather than reading both implementations.

## Week 8 — `admet-infer`, `admet-db`, `admet-api`; parity green · tag `v0.2.0-api`

**Read.** Manual ch. 19 (Axum) with 19.2–19.4, ch. 20.3 (repositories, bulk insert),
26.3–26.5 (config, health, tracing), and 22 (CLI).

**Touch.** `crates/admet-infer/`, `crates/admet-db/` + `migrations/0001_initial.sql`,
`crates/admet-api/`, `crates/admet-cli/`.

**Run.**

```bash
just db-up && just db-migrate
```

```bash
export DATABASE_URL="postgres://admet:changeme@localhost:5433/admet"
```

```bash
just serve
```

```bash
curl -X POST localhost:8080/predict -H 'content-type: application/json' -d '{"smiles":"CC(=O)Oc1ccccc1C(=O)O"}'
```

Then commit the query metadata so CI can build without a database:

```bash
cargo sqlx prepare --workspace
```

**Done when.**

- [ ] `/predict` returns all twelve endpoints in **one** response (FR-08)
- [ ] End-to-end parity Python↔Rust to 1e-4, running **in CI** (TR-04) — `TC-I-050…059`
- [ ] Errors are RFC 9457 problem details (TR-09); out-of-domain is a **200 carrying a refusal**, not an error code (FR-12)
- [ ] 20 MB body cap and 30 s timeout enforced (TR-06) — `TC-S-001…005`
- [ ] Cache keyed `(inchikey, model_version)` (TR-05), and a second identical request is served from it
- [ ] `just bench-cli N=1000` reports a p95, recorded in `results/` as a measurement
- [ ] `.sqlx/` committed in the same commit as the queries it describes
- [ ] `docs/journal/week-08.md` and `v0.2.0-api` pushed

**Feeds.** Report implementation and evaluation chapters; NFR-01 latency evidence.

## Week 9 — SvelteKit workspace, single-molecule report · Increment 3

**Touch.** `web/` — SvelteKit 2 / Svelte 5 runes, UnoCSS, SmilesDrawer.

**Run.** Uncomment the `web` job in `.github/workflows/ci.yml` this week, not before —
a red badge from week one teaches you to ignore CI.

```bash
corepack enable pnpm && cd web && pnpm install
```

```bash
just web
```

**Done when.**

- [ ] SMILES input with quick-select examples: aspirin, caffeine, ibuprofen, propranolol
- [ ] 2D depiction renders (FR-13)
- [ ] Deterministic values are **visually distinct** from predicted ones, with a visible domain-confidence badge (NFR-10)
- [ ] An invalid SMILES shows an inline message at the offending character, not a blank panel
- [ ] The `web` CI job is green
- [ ] `docs/journal/week-09.md`

**Feeds.** Report implementation chapter; the NFR-10 screenshot in
[`docs/evidence/`](docs/evidence/README.md), which is the only acceptable evidence for
that requirement because no test can assert "a chemist can tell these apart".

## Week 10 — projects, auth, NFR-10 evidence; Increment 4 starts · tag `v0.3.0-web`

**Done when.**

- [ ] Named projects and saved molecule sets (FR-23), passwords hashed with **Argon2id**
- [ ] Observable Plot distribution charts
- [ ] A chemist completes UC-01 in a browser without touching `curl` — `TC-SYS-001…009`
- [ ] `docs/evidence/` holds the dated, commit-stamped NFR-10 screenshot with its caption file
- [ ] `docs/journal/week-10.md` and `v0.3.0-web` pushed

**Before you write the auth code:** it is the one part of this system where a shortcut has
consequences outside the project. Argon2id with per-user salts, no session token in
`localStorage`, and no password in a log line. Nothing about this is negotiable because
the deployment is small.

## Week 11 — batch pipeline, streaming progress, triage ranking · Increment 4

**Read.** Manual ch. 14 for desirability, and re-read
[`triage.rs`](crates/admet-core/src/triage.rs)'s module docs.

**Run.**

```bash
just bench-cli N=10000
```

**Done when.**

- [ ] `POST /predict/batch` takes CSV up to 50,000 rows (FR-14) through a **bounded** channel (TR-08) — memory flat, not growing
- [ ] Checkpoint every 250 rows, bulk insert via `UNNEST` with `ON CONFLICT DO NOTHING` (FR-16)
- [ ] Progress streams to the client (FR-15) — `TC-I-020…029`
- [ ] Triage score is a **geometric** mean (FR-17), so one disqualifying endpoint sinks the candidate; top-k via min-heap (FR-18)
- [ ] Molecules out of domain return a **null** triage score with a reason, never a guess (FR-12)
- [ ] 10,000 molecules in under 90 s on 2 vCPU, measured with `nproc` evidence (NFR-02)
- [ ] `docs/journal/week-11.md`

**Why geometric and not arithmetic:** both return values in `[0,1]`, so a range check
would never notice the substitution — but an arithmetic mean lets eleven good endpoints
hide one fatal `herg` result, which is precisely the molecule the tool exists to catch.

## Week 12 — attribution, export; Increment 5 starts · tag `v0.4.0-batch`

**Touch.** `training/models/explain.py`, the attribution route, the export path.

**Done when.**

- [ ] Per-atom attribution via integrated gradients (FR-19) — `TC-U-080…089`
- [ ] Colour-mapped overlay on the 2D depiction (FR-20), with the colour scale in the legend
- [ ] Comparison against approved-drug distributions (FR-21), reusing the ~500-structure applicability-domain reference set so it is not extra work
- [ ] CSV **and** PDF export (FR-22)
- [ ] A 1,000-molecule CSV screens end to end and returns a ranked table in the browser
- [ ] `docs/journal/week-12.md` and `v0.4.0-batch` pushed

**Fold in the chemistry credibility layer here** if it is not already done
([`implementation.md`](implementation.md) §6): QED, per-rule Lipinski/Veber/Ghose, PAINS
and Brenk alerts returning the **name** of each matched substructure rather than a
boolean, SA score, nearest known drug. It is pure RDKit, no training, and it is what makes
the deterministic half of the report read as though written by someone who knows
medicinal chemistry.

Sanity check that catches most of its bugs at once: predicting aspirin should return a
QED, per-rule pass/fail, zero PAINS alerts, an SA score, and *"closest known drug:
aspirin, similarity 1.00"*.

## Week 13 — Tauri desktop, Docker, release automation · tag `v0.5.0-desktop`

**Read.** Manual ch. 26.2, and [troubleshooting §17](docs/reference/troubleshooting.md)
before the first Tauri build.

**Run.**

```bash
winget install --id Microsoft.EdgeWebView2Runtime
```

```bash
just desktop
```

**Done when.**

- [ ] Tauri v2 shell wraps the **same** UI — a wrapper, not a second frontend, which is what [ADR-02](docs/adr/0002-hexagonal-crate-split.md) was for
- [ ] Offline mode with bundled SQLite (FR-24) — verified with the network adapter **disabled**, and `ipconfig` output committed as evidence
- [ ] Multi-stage Docker build to a distroless image; `docker stats` shows < 512 MB RSS (NFR-12)
- [ ] Cold start under 10 s (NFR-11); installer under 15 MB, evidenced by `ls -l`
- [ ] CI runs fmt, clippy, nextest, llvm-cov, audit, deny, about, Vitest, Playwright (TR-11)
- [ ] Coverage ≥ 75 % on `admet-core` + `admet-infer` (NFR-04): `just coverage`
- [ ] Commit to deployed in under 10 minutes, unattended
- [ ] `docs/journal/week-13.md` and `v0.5.0-desktop` pushed

**Risk R8 is why this is tested on Windows from week one.** A Tauri build that has never
run on the development platform is discovered to be broken in the last week, with no time
to find out whether the cause is Tauri, WebView2, or the bundle configuration.

## Week 14 — system and acceptance testing; traceability matrix closed

**Week 14 is not slack.** It is the week the traceability matrix stops being a list of
intentions, and it is the only week whose entire output is documentation. A project
running late will try to spend it on features; the tag schedule above exists so that
becomes visible in advance rather than in week 15.

**Read.** [`03-test-plan.md`](docs/03-test-plan.md) end to end, and
[`04-traceability.md`](docs/04-traceability.md) §9, whose gap table is the checklist for
this week.

**Run.**

```bash
just ci-local
```

```bash
comm -23 <(grep -oE 'TC-[UISYSP]+-[0-9]+' docs/04-traceability.md | sort -u) <(git grep -hoE 'TC-[UISYSP]+-[0-9]+' -- crates training | sort -u)
```

That second command returns all 61 ids today. **Reaching empty output is the definition
of the matrix being true**, and it is this week's single most important number.

**Done when.**

- [ ] Every UC-01…UC-08 has a passing acceptance test
- [ ] The `comm -23` check returns nothing, or every remaining id is listed in §9 with a reason
- [ ] The register `diff` check returns nothing
- [ ] Every `DEF-nn` in §10.2 has an answer to *"which test would have caught this?"*
- [ ] `results/` regenerates from scratch with `just results` — no hand-typed figures anywhere
- [ ] `docs/journal/week-14.md`

**Feeds.** Report testing chapter and the appendix traceability matrix — the two sections
where a reader checks whether the rest of the document is trustworthy.

## Week 15 — deployment, documentation freeze, report · tag `v1.0.0`

**Done when.**

- [ ] Deployed instance reachable, behind Caddy with automatic TLS
- [ ] `just report` compiles the PDF; every figure in it came from `just results` or `just diagrams`
- [ ] Fifteen journal entries exist, including stubs for any week that did not happen
- [ ] The evaluation chapter names the targets that were **missed** and why
- [ ] `v1.0.0` pushed

**The evaluation chapter is the graded one.** A retrospective assembled from a clean
commit log always says the same three things — the project went broadly to plan, Rust had
a learning curve, more time for testing would have helped — and none of it is worth
reading. What is worth reading is the afternoon lost to a CRLF-mangled `.f32` fixture.
That detail survives about three days in memory, which is why
[`docs/journal/`](docs/journal/README.md) exists and why §5 of each entry records hours
lost per defect.

---

## 5. When you fall behind — decide the cut order now

You will lose a week somewhere; risk R9 assumes it. What matters is that the cut is
chosen in advance, when it is an engineering decision, rather than in week 13 when it is a
panic.

**Increments 1–3 alone constitute a complete, presentable system.** So the cut order is
fixed, worst thing to lose last:

| Cut first | What survives |
|---|---|
| 1. Desktop packaging (Increment 5) | Web app still demonstrates everything; FR-24 and NFR-11/12 are documented as unmet |
| 2. Batch + attribution (Increment 4) | Single-molecule UC-01 is intact and the demonstration stays coherent |
| 3. Twelve endpoints → fewer | The vertical slice still works end to end; report the endpoints you have |
| **Never** | The parity test, the scaffold split, the traceability matrix, the journal |

Those last four are what make the *rest* believable. A project with four endpoints and an
honest, traced, tested pipeline reads far better than twelve endpoints whose numbers
nobody can reproduce.

Record the cut as a decision in that week's journal §4, and mark the affected requirement
rows in [`04-traceability.md`](docs/04-traceability.md) as unmet rather than deleting
them. A requirement that was consciously dropped is a finding; one that quietly vanished
looks like something you forgot.

## 6. The daily loop

```bash
just test-core
```

About a second, no database, no model — run it constantly.

```bash
just ci-local
```

Before every push. It is the same gate CI runs, so if it is green locally CI will be too.

Commit style, from [`05-git-conventions.md`](docs/05-git-conventions.md): conventional
type and scope, plus a `Refs:` trailer naming the requirement or defect. That trailer is
what makes the history queryable:

```bash
git log --oneline --grep="FR-17"
```

## 7. When something breaks

In this order, cheapest first:

1. **`just verify`** — a surprising share of failures are a missing tool, reported three
   layers down as something unrelated.
2. **`just spike`** for anything ONNX (seconds, no data); **`just test-core`** for
   anything chemistry (about a second). If both are green, the problem is in an adapter,
   not the domain.
3. **[`docs/reference/troubleshooting.md`](docs/reference/troubleshooting.md)**, indexed
   by the literal error text. `Ctrl-F` the message.
4. **Check the artefact, not the code** — file sizes, opset versions, RDKit versions,
   `git check-attr`. Half of the failures on this platform are artefacts, not logic.
5. **Then add the entry** to `troubleshooting.md` with the exact message. A page written
   from memory in week 15 contains the four problems that were memorable, not the twenty
   that cost time.

## 8. Where everything is

| Looking for | Go to |
|---|---|
| A command | [`docs/reference/commands.md`](docs/reference/commands.md) |
| A term you do not know | [`docs/reference/glossary.md`](docs/reference/glossary.md) |
| An error message | [`docs/reference/troubleshooting.md`](docs/reference/troubleshooting.md) |
| Which manual chapter covers a file | [`docs/reference/build-manual-index.md`](docs/reference/build-manual-index.md) |
| Why a decision was made | [`docs/adr/`](docs/adr/README.md) |
| What a requirement id means | [`requirements.md`](requirements.md), then [`docs/01-srs.md`](docs/01-srs.md) |
| Whether something is tested | [`docs/04-traceability.md`](docs/04-traceability.md) |
| What happened in week *n* | [`docs/journal/`](docs/journal/README.md) |

---

Start with §2. Nothing else can be verified until `cargo` exists.











