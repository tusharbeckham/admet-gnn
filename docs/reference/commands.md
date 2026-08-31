# Command reference

| | |
|---|---|
| **System** | ADMETriage |
| **Document** | Every command this project needs, and the raw form behind each recipe |
| **Status** | Current at the scaffold tag. Recipes marked ⚠ have nothing to run yet. |
| **Traces** | [`justfile`](../../justfile) · [`00-machine-setup.md`](../00-machine-setup.md) |

Everything routine is a `just` recipe, so the command is identical on this laptop and
in CI. This page exists for the two cases a recipe cannot cover: when you need to know
*what* a recipe runs in order to vary it, and when `just` itself is not installed yet.

All commands assume **Git Bash on Windows** from the repository root
([ADR-07](../adr/0007-native-windows-and-training-layout.md)). The venv interpreter is
named explicitly as `.venv/Scripts/python.exe` rather than relying on `python`, because
half of all "it worked yesterday" reports on Windows are a shell that forgot it had a
venv activated.

## 1. First contact

```bash
just --list
```

```bash
just verify
```

`just verify` runs [`scripts/verify-env.sh`](../../scripts/verify-env.sh) and prints
`OK` or `MISSING` per tool with a non-zero exit if anything is absent. **Red rows are
the script working**, not a broken repo — install what it names from
[`00-machine-setup.md`](../00-machine-setup.md).

## 2. Environment

| Recipe | Raw command | Notes |
|---|---|---|
| `just setup` | `uv venv .venv --python 3.12` then `uv pip install --python .venv/Scripts/python.exe -r requirements.txt` | Also installs git hooks. Does **not** install rustup/just/pnpm — those bootstrap the thing that would install them. |
| `just hooks` | `git config core.hooksPath .githooks` | Versioned hooks. Skip once with `git commit --no-verify`. |
| `just setup-tdc` | `uv venv .venv-tdc --python 3.12` + `-r requirements-data.txt` | Separate env: PyTDC pins `rdkit<2024.3.1`, this project runs 2026.3.5 (risk R3). |

## 3. Data — Increment 1

| Recipe | Raw command | Notes |
|---|---|---|
| `just data-download` | `.venv-tdc/Scripts/python.exe training/data/download_tdc.py` | Raw CSV only. **Zero chemistry** — the hard rule in `requirements-data.txt`. |
| `just data-profile` | `.venv/Scripts/python.exe training/data/profile.py` | Run **before** any modelling. Where you learn that an endpoint is log-transformed, or that 3 % of a set exceeds the 128-atom cap. |
| `just data-prepare` | `clean.py` then `scaffold_split.py` | One pinned RDKit for both. |

## 4. Model — Increment 1

| Recipe | Raw command | Notes |
|---|---|---|
| `just spike` | `.venv/Scripts/python.exe training/scripts/spike_onnx_export.py` | The week-1 gate. Untrained dense GIN → ONNX → reload → outputs match across batch 1/7/64. Needs no data, takes seconds. |
| `just train` | `python -m training.train` ⚠ | Accepts pass-through args: `just train --endpoint bbb --seed 0`. |
| `just export` | `python -m training.export` ⚠ | Writes `models/model.onnx` and verifies it loads. |
| `just parity-fixture` | `python training/scripts/dump_parity_fixture.py` | Regenerates `fixtures/parity/`. The only thing keeping the two featurisers honest (TR-03). |
| `just schema` | `cargo run -q -p admet-cli -- schema > models/feature_schema.json` | **Rust emits, Python asserts.** Direction matters: the Rust featuriser is the contract. |

## 5. Rust

| Recipe | Raw command |
|---|---|
| `just build` | `cargo build --workspace` |
| `just build-release` | `cargo build --workspace --release` — use for any number that goes in the report |
| `just serve` | `ADMET_PROFILE=local cargo run -p admet-api` |
| `just predict SMILES` | `cargo run -q -p admet-cli -- predict "<smiles>"` ⚠ — no server, no database, no cache |
| `just bench-cli N=1000` | `cargo run -q --release -p admet-cli -- bench --n 1000 --breakdown` ⚠ |
| `just bench` | `cargo bench -p admet-core` |

Useful raw forms with no recipe:

```bash
cargo build -p admet-core
```

```bash
cargo tree -p admet-api --depth 2
```

```bash
cargo test -p admet-core -- --nocapture parser
```

## 6. Quality gates

```bash
just check
```

That is `fmt-check` + `lint` + `audit` — exactly what CI runs, in the order that fails
fastest. If it is green locally, CI will be green.

| Recipe | Raw command | Notes |
|---|---|---|
| `just fmt` | `cargo fmt --all` | |
| `just fmt-check` | `cargo fmt --all -- --check` | |
| `just lint` | `cargo clippy --workspace --all-targets -- -D warnings` | `-D warnings` is the point: a warning nobody must fix is a warning nobody reads, and by week six there are two hundred. |
| `just audit` | `cargo audit` | |
| `just test` | `test-rust` + `test-py` | |
| `just test-rust` | `cargo nextest run --workspace`, falling back to `cargo test --workspace` | nextest for the JUnit XML the test-report chapter needs. |
| `just test-core` | `cargo test -p admet-core` | The fast inner loop, ~1 s. What the pre-commit hook runs, and why [ADR-02](../adr/0002-hexagonal-crate-split.md) pays for itself. |
| `just test-py` | `python -m pytest training/tests -q` | |
| `just coverage` | `cargo llvm-cov --workspace --html` | Opens at `target/llvm-cov/html/index.html`. NFR-04 is ≥ 75 % on core + infer. |
| `just ci-local` | `check` then `test` | Run before every push. |

## 7. Database — Increment 2

| Recipe | Raw command | Notes |
|---|---|---|
| `just db-up` | `docker run -d --name admet-postgres … -p 5433:5432 postgres:16-alpine` | Port **5433**, chosen to avoid a system Postgres. Waits for `pg_isready`. |
| `just db-down` | `docker stop` + `docker rm` | |
| `just db-shell` | `docker exec -it admet-postgres psql -U admet -d admet` | Inside the container, so a native `psql` is optional. |
| `just db-migrate` | `sqlx migrate run --database-url …` | Clean no-op until Increment 2 writes `migrations/0001_initial.sql`. |
| `just db-reset` | `db-down` → `db-up` → `db-migrate` | **Destroys all data.** Separate from `db-up` so it is never something you get by accident. |

`sqlx` needs the URL at compile time for its checked queries:

```bash
export DATABASE_URL="postgres://admet:changeme@localhost:5433/admet"
```

## 8. Frontend and desktop — Increments 3 and 5

| Recipe | Raw command |
|---|---|
| `just web` | `cd web && pnpm dev` ⚠ |
| `just web-build` | `cd web && pnpm build` ⚠ |
| `just desktop` | `cd desktop && pnpm tauri dev` ⚠ |

## 9. Report artefacts

| Recipe | Raw command | Notes |
|---|---|---|
| `just results` | `python -m training.report` + `cargo bench -p admet-core -- --save-baseline report` ⚠ | Regenerates **every** number in `results/`. If a figure cannot be produced by this recipe, it is a target and must be labelled as one. |
| `just diagrams` | `python -m training.viz.export_diagrams` ⚠ | Exports the generated figures 11–15 from [`diagrams/README.md`](../diagrams/README.md). |
| `just report` | `typst compile docs/report/main.typ docs/report/ADMETriage.pdf` ⚠ | |

## 10. Git

```bash
git log --oneline --grep="DEF-"
```

The defect history in order — see [`05-git-conventions.md`](../05-git-conventions.md)
for the `Refs:` trailer that makes this work.

```bash
git log --oneline --grep="FR-04"
```

Every commit that touched a requirement.

```bash
git tag -a v0.0.1-scaffold -m "Scaffold: five crates build clean, CI green"
```

```bash
git push --follow-tags
```

## 11. The two traceability self-checks

From [`04-traceability.md`](../04-traceability.md) §1. Run both before cutting any
tag; a line of output from either means a document is claiming something untrue.

```bash
diff <(grep -oE '(FR|TR|NFR|UC)-[0-9]+' docs/04-traceability.md | sort -u) <(grep -oE '(FR|TR|NFR|UC)-[0-9]+' requirements.md | sort -u)
```

```bash
comm -23 <(grep -oE 'TC-[UISYSP]+-[0-9]+' docs/04-traceability.md | sort -u) <(git grep -hoE 'TC-[UISYSP]+-[0-9]+' -- crates training | sort -u)
```

The second returns all 61 cited test IDs today, because none of them exist yet. It
should shrink every week from Increment 1 onward, and reaching empty is the definition
of the matrix being true.
