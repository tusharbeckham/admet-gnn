# =============================================================================
#  ADMETriage task runner
#
#  Manual Listing 4.3, adapted for native Windows + Git Bash (ADR-07) and
#  extended with this repo's own vocabulary from implementation.md §9.
#
#      just              list every recipe
#      just verify       is this machine ready?
#      just check        fmt + clippy + audit  (what CI runs)
#      just test         both languages
#
#  Why a justfile rather than a README full of commands: a command in prose gets
#  typed slightly differently each time, and the fourth variation is the one that
#  wastes an afternoon. A recipe is the command, versioned.
# =============================================================================

# Git Bash, not cmd.exe. `-u` makes an unset variable an error instead of an
# empty string -- the difference between "cargo test -p " failing loudly and it
# silently testing the whole workspace.
set shell := ["bash", "-uc"]

#  Windows needs the path spelled out, and this is not pedantry. On a machine
#  with the WSL optional feature enabled, bare `bash` resolves to
#  C:\windows\system32\bash.exe -- the WSL LAUNCHER -- which drops every recipe
#  into a different operating system with a different filesystem and no Rust
#  toolchain. The symptom is `cargo: command not found` from `just check` while
#  `cargo --version` works fine in the same terminal, which is a genuinely
#  confusing half hour.
#
#  ADR-07 chose native Windows + Git Bash over WSL2; this line is what actually
#  enforces that choice. `set shell` above still governs Linux CI runners, where
#  bare `bash` is correct.
set windows-shell := ["C:/Program Files/Git/bin/bash.exe", "-uc"]

# --- paths -------------------------------------------------------------------
# The venv interpreter is named explicitly rather than assuming `python` is
# active. Half of all "it worked yesterday" reports on Windows are a shell that
# forgot it had a venv activated.
py       := ".venv/Scripts/python.exe"
py_tdc   := ".venv-tdc/Scripts/python.exe"
pg_url   := env_var_or_default("ADMET_DATABASE__URL", "postgres://admet:changeme@localhost:5433/admet")
pg_ctr   := "admet-postgres"

# Default recipe: show what is available.
default:
    @just --list --unsorted

# =============================================================================
#  Environment
# =============================================================================

# Check every tool this project needs and say which are missing.
verify:
    bash scripts/verify-env.sh

# One-time machine setup: Python env, Node deps, git hooks.
#
# Does NOT install rustup, just, or pnpm -- those bootstrap the thing that would
# be installing them. See docs/00-machine-setup.md for those four commands.
setup: hooks
    uv venv .venv --python 3.12
    uv pip install --python {{py}} -r requirements.txt
    @echo "next: just verify"

# Point git at the versioned hooks directory.
#
# `core.hooksPath` rather than copying into .git/hooks: a copied hook is
# unversioned, so it drifts, and nobody notices it stopped running.
hooks:
    git config core.hooksPath .githooks
    @echo "hooks installed from .githooks/ (skip once with: git commit --no-verify)"

# Create the throwaway TDC environment. Separate on purpose -- PyTDC pins
# rdkit<2024.3.1 and this project runs 2026.3.5. Risk R3.
setup-tdc:
    uv venv .venv-tdc --python 3.12
    uv pip install --python {{py_tdc}} -r requirements-data.txt
    @echo "next: just data-download"

# =============================================================================
#  Data  (Increment 1, week 1)
# =============================================================================

# Download the twelve TDC endpoints as raw CSV. Zero chemistry -- see the HARD
# RULE in requirements-data.txt.
data-download:
    {{py_tdc}} training/data/download_tdc.py

# Profile every raw CSV: rows, unparseable count, heavy-atom distribution,
# fraction above the 128-atom cap, target mean/std.
#
# Run this BEFORE any modelling. It is where you find out that a "regression"
# endpoint is log-transformed, or that 3 % of a set exceeds the cap.
data-profile:
    {{py}} training/data/profile.py

# Clean and scaffold-split. Runs in the training env against one pinned rdkit.
data-prepare:
    {{py}} training/data/clean.py
    {{py}} training/data/scaffold_split.py

# =============================================================================
#  Model  (Increment 1)
# =============================================================================

# The ONNX round-trip spike: does a dense-adjacency GIN export and reload with
# matching outputs across batch sizes? This is the gate that justified ADR-03.
spike:
    {{py}} training/scripts/spike_onnx_export.py

# Train every endpoint.
train *ARGS:
    {{py}} -m training.train {{ARGS}}

# Export to models/model.onnx and verify the artefact loads.
export:
    {{py}} -m training.export

# Regenerate the parity fixture the Rust side asserts against (TR-03).
#
# The featuriser exists in two languages; this is the only thing that keeps them
# honest. A disagreement here is a wrong prediction, not a crash.
parity-fixture:
    {{py}} training/scripts/dump_parity_fixture.py

# =============================================================================
#  Rust
# =============================================================================

# Debug build of every crate.
build:
    cargo build --workspace

# Release build. Used for any number that goes in the report.
build-release:
    cargo build --workspace --release

# Run the API against config/local.toml.
serve:
    ADMET_PROFILE=local cargo run -p admet-api

# Score one molecule with no server, no database, no cache.
predict SMILES:
    cargo run -q -p admet-cli -- predict "{{SMILES}}"

# Export the 33-feature schema for the Python featuriser to read.
schema:
    cargo run -q -p admet-cli -- schema > models/feature_schema.json
    @echo "wrote models/feature_schema.json"

# Benchmark the pipeline end to end, with no HTTP in the way.
bench-cli N="1000":
    cargo run -q --release -p admet-cli -- bench --n {{N}} --breakdown

# Criterion benchmarks over the pure functions.
bench:
    cargo bench -p admet-core

# =============================================================================
#  Quality gates
# =============================================================================

# What CI runs. If this is green locally, CI will be green.
#
#  That sentence is a promise, and it was false twice on the first real CI run:
#  `check` omitted `cargo doc` and the Python lint, so a broken intra-doc link
#  (`RingTable::close`, which does not exist) and two `EXE001` findings reached
#  GitHub instead of being caught here. `EXE001` cannot fire on Windows at all --
#  there is no executable bit to be missing -- which is exactly why the local gate
#  has to run the same commands rather than a convenient subset.
check: fmt-check lint docs-check lint-py audit

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

# `-D warnings` is the point. A warning nobody has to fix is a warning nobody
# reads, and by week six there are two hundred of them.
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Rustdoc as a GATE, not a nicety. `-D warnings` turns a broken intra-doc link
# into a failure, which matters because these documents are graded: a design doc
# that links to a method that does not exist is wrong about the code it describes.
docs-check:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items

# Python lint AND format, both in check mode -- CI runs both, and `ruff check`
# alone passes on badly formatted code.
lint-py:
    {{py}} -m ruff check .
    {{py}} -m ruff format --check .

audit:
    cargo audit

# Full suite, both languages.
test: test-rust test-doc test-py

# Doc tests, which `cargo nextest` does NOT run -- nextest has no doctest support,
# so a suite that only uses nextest silently skips every example in the docs. CI
# runs this as its own step for the same reason.
test-doc:
    cargo test --workspace --doc

# nextest for the JUnit XML that the test-report chapter needs; falls back to
# `cargo test` if nextest is not installed yet.
test-rust:
    #  Deliberately NOT a `#!/usr/bin/env bash` shebang recipe. `just` implements
    #  shebang recipes by writing a temp script and translating its path with
    #  `cygpath`, which lives in Git's `usr/bin` and is usually absent from the
    #  Windows PATH -- the failure is `could not find 'cygpath' executable`, which
    #  says nothing about the actual cause. A normal recipe runs through
    #  `windows-shell` (Git Bash) and needs no translation at all.
    if cargo nextest --version >/dev/null 2>&1; then cargo nextest run --workspace; else echo "note: cargo-nextest not installed, falling back to cargo test" >&2; cargo test --workspace; fi

# The fast inner loop: admet-core only, no I/O, no database, no model.
# This is what the pre-commit hook runs, and why ADR-02 pays for itself.
test-core:
    cargo test -p admet-core

test-py:
    {{py}} -m pytest training/tests -q

# Coverage over the workspace.
coverage:
    cargo llvm-cov --workspace --html
    @echo "open target/llvm-cov/html/index.html"

# NFR-04 as a GATE, with the requirement's own scope.
#
#  The floor is "≥ 75% on `admet-core` and `admet-infer`", NOT on the workspace.
#  That distinction is the whole point: the workspace total is 74.88% and would
#  fail, but it is dragged down by `admet-db` repository stubs that cannot be
#  covered without a live Postgres. Gating on the workspace number would either
#  fail honestly-written code or push someone to write hollow database tests to
#  move a percentage. Gating on the two pure crates measures what NFR-04 actually
#  claims: the logic that produces the numbers is exercised.
#
#  Measured at the scaffold tag: 83.14% regions / 83.86% lines.
coverage-gate:
    cargo llvm-cov --package admet-core --package admet-infer \
        --summary-only --fail-under-lines 75

# =============================================================================
#  Database  (Increment 2)
# =============================================================================

# Postgres 16 on 5433. Port chosen to avoid colliding with a system Postgres.
db-up:
    docker run -d --name {{pg_ctr}} \
        -e POSTGRES_USER=admet -e POSTGRES_PASSWORD=changeme -e POSTGRES_DB=admet \
        -p 5433:5432 postgres:16-alpine
    @echo "waiting for postgres..."
    @until docker exec {{pg_ctr}} pg_isready -U admet -q; do sleep 1; done
    @echo "ready: {{pg_url}}"

db-down:
    -docker stop {{pg_ctr}}
    -docker rm {{pg_ctr}}

# Interactive psql inside the container, so a native psql is optional.
db-shell:
    docker exec -it {{pg_ctr}} psql -U admet -d admet

# Apply migrations. A clean no-op until Increment 2 writes 0001_initial.sql.
db-migrate:
    #  Same reason as `test-rust`: no shebang, so no `cygpath` requirement.
    if ! compgen -G "migrations/*.sql" >/dev/null; then echo "no migrations yet -- nothing to apply"; else sqlx migrate run --database-url "{{pg_url}}"; fi

# Destroys all data. Named `db-reset` rather than folded into db-up so it is
# never something you get by accident.
db-reset: db-down db-up db-migrate

# =============================================================================
#  Frontend  (Increment 3) and desktop (Increment 5)
# =============================================================================

web:
    cd web && pnpm dev

web-build:
    cd web && pnpm build

desktop:
    cd desktop && pnpm tauri dev

# =============================================================================
#  Report artefacts
# =============================================================================

# Regenerate every number in results/.
#
# This matters more than it looks: it means no figure in the report is ever
# hand-copied, so nothing silently drifts out of date between the code and the
# document. If a number cannot be produced by this recipe, it is a target and
# must be labelled as one.
results:
    {{py}} -m training.report
    cargo bench -p admet-core -- --save-baseline report

# Export diagram sources to docs/diagrams/. 300 DPI raster or SVG, per ch. 27.
diagrams:
    {{py}} -m training.viz.export_diagrams

# Build the report PDF.
report:
    typst compile docs/report/main.typ docs/report/ADMETriage.pdf

# =============================================================================
#  Housekeeping
# =============================================================================

clean:
    cargo clean
    -rm -rf target/llvm-cov data/processed/__pycache__

# Everything a pull request needs to pass, in the order that fails fastest.
ci-local: check test
    @echo "OK -- safe to push"
