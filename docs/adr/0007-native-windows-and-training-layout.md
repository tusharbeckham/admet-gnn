# ADR-07: Develop on native Windows, and keep the Python code in `training/`

- **Status**: Accepted
- **Date**: 2026-08-18
- **Deciders**: tusharbeckham
- **Related**: NFR-07, NFR-09, NFR-12, G7, R8

## Context

This ADR exists for a specific reason: it records the **two places this project
deliberately departs from the build manual**. Both are defensible, both cost
something, and an undocumented deviation is indistinguishable from having not read
the manual — so they are written down here rather than left implicit in the tree.

### Deviation 1 — platform

The build manual §2.1 advises WSL2 on Windows, and the advice is sound in general:
the Rust and Postgres tooling was built on Linux, most documentation assumes it,
and container workflows are smoother.

The situation here is that the Python half **already works on native Windows**:
`.venv` on Python 3.12.10 with torch 2.13.0+cpu, onnx 1.22, onnxruntime 1.29 and
rdkit 2026.3.5, and a verified ONNX export spike. Moving to WSL2 means rebuilding
that environment, and the specific costs are concrete rather than hypothetical:

- **Filesystem performance across the boundary.** Working in `/mnt/c` is slow enough
  to be noticeable on a `cargo build`, and the alternative — keeping the repo inside
  the WSL2 filesystem — means Windows-side editors and tools reach it over a network
  share.
- **Two toolchains, indefinitely.** Tauri's Windows bundle (Increment 5) must be
  built on Windows with MSVC regardless. So WSL2 does not replace the native
  toolchain; it adds a second one to keep working.
- **GPU passthrough.** Not needed here — training is CPU-only on ~37,000 rows — but
  it is the usual reason to accept WSL2's costs, and it does not apply.

### Deviation 2 — Python layout

The manual specifies `ml/src/admet_ml/` — a `src`-layout installable package, which
is the correct structure for a library that will be `pip install`ed by someone else.

This repository already has `training/`, with git history, and
`implementation.md` §8 documents that layout. The Python code here is not a
distributable library: it is a set of scripts that produce one ONNX file. It is
never imported by a third party, never published to PyPI, and never versioned
independently.

## Decision

**Develop on native Windows 11 with Git Bash** as the shell. Not WSL2.

`scripts/verify-env.sh` targets Git Bash and resolves the interpreter explicitly as
`.venv/Scripts/python.exe`. A PowerShell twin, `scripts/verify-env.ps1`, exists with
identical checks and exit codes. The [`justfile`](../../justfile) sets
`set shell := ["bash", "-uc"]`.

**Keep the Python code in `training/`**, flat, not `ml/src/admet_ml/`:

```text
training/
  data/        download_tdc.py  profile.py  clean.py  scaffold_split.py
  features/    the 33-dim featuriser
  models/      the dense GIN
  scripts/     spike_onnx_export.py  dump_parity_fixture.py
  tests/
  legacy_moleculenet/   the superseded prototype, kept as evidence
```

## Consequences

### Positive

- **The working Python environment is preserved.** Verified torch/onnx/rdkit versions
  and a passing export spike, untouched. Week one is not spent rebuilding what
  already works.
- **One toolchain, not two.** The MSVC toolchain needed for Tauri in Increment 5 is
  the same one used from day one, so the desktop build is not the first time it gets
  exercised.
- **Native paths everywhere.** Editor, debugger, Docker Desktop and the Rust
  toolchain all see the same filesystem with the same paths. No `/mnt/c` translation
  to reason about when a path in a log looks wrong.
- **Windows is exercised continuously, which is the point for G7.** The desktop build
  targets Windows; developing there means platform-specific problems surface in week
  three rather than week fourteen. The absence of `SIGTERM` on Windows is already
  handled in `crates/admet-api/src/main.rs` for exactly this reason.
- **`training/` matches git history and `implementation.md`.** No rename commit that
  makes every prior commit's paths wrong, and no divergence between the documented
  layout and the actual one.

### Negative

- **Almost every Rust and Postgres example is written for Linux.** Path separators,
  `export VAR=` versus `set`, `sudo apt` install lines. Each is a small friction and
  they accumulate; the mitigation is that [`docs/00-machine-setup.md`](../00-machine-setup.md)
  gives the Windows command for every tool rather than leaving it as an exercise.
- **CI runs `ubuntu-latest`, so CI and the laptop are different platforms.** This is a
  real gap: a path-handling bug that only appears on one of them is possible.
  Accepted deliberately, because ubuntu runners are faster and cheaper and the
  production target is a Linux container anyway. If a platform-specific bug appears,
  add a `windows-latest` matrix entry then — adding it speculatively doubles CI time
  for a problem that may not exist.
- **`sqlx` offline mode matters more here.** Compile-time-checked queries need either
  a live database or a committed `.sqlx/` directory. On Windows, with Postgres in a
  container, the live-database path is the more fragile one, so the project uses
  runtime-checked queries plus a committed `.sqlx/` when the macros arrive in
  Increment 2.
- **Docker Desktop is required for Postgres**, and it is heavier on Windows than the
  Linux daemon. `just db-shell` runs `psql` *inside* the container so a native `psql`
  install is optional.
- **`training/` is not an installable package.** No `pyproject.toml`, so `pip install
  -e .` does not work and `python -m training.train` depends on being run from the
  repository root. That is a genuine limitation. It is acceptable because the
  [`justfile`](../../justfile) is the entry point for every Python command and it
  always runs from the root — but if this code ever needs to be a library, the
  manual's layout becomes correct and this ADR gets superseded.
- **Line endings need attention.** `.gitattributes` must keep `.sh` files LF, or the
  git hook fails on Windows with a `\r` in the shebang and an error message that
  names neither the cause nor the file.

### Neutral

- `rust-toolchain.toml` pins the `stable` channel with `rustfmt` and `clippy` rather
  than a version number. That is a separate small deviation (the manual pins a
  version), documented in the file itself: a pinned version means rustup downloads a
  second toolchain and then rots for fifteen weeks, whereas the components are the
  part that is actually load-bearing because `cargo fmt` and `cargo clippy` are in
  the definition of done.
- Node is pinned to **22** via `.nvmrc` even though v24.14.1 is installed, so CI and
  the laptop agree. See `docs/00-machine-setup.md`.
- `lto = "thin"` rather than the manual's `"fat"`, because ch. 24.2 lists
  LTO + codegen-units as an optimisation to be *measured*. Leaving it thin means
  there is a real before/after number to report instead of an assumed one.

## Alternatives considered

### WSL2, as the manual advises

The mainstream answer and it would work. Rejected on the balance above: it requires
rebuilding a verified Python environment, does not remove the need for a native MSVC
toolchain for Tauri, and its main advantage (GPU passthrough) is irrelevant to a
CPU-only training run. Worth reconsidering if a Linux-only dependency appears that
cannot be worked around — that would be a superseding ADR, not a quiet migration.

### Dev container

Reproducible, and it would make CI and local identical. Rejected because it puts the
Rust toolchain inside a container while Tauri's bundle must be built outside it, so
the build story splits in two — and because a container's filesystem performance on
Windows has the same `/mnt/c`-shaped problem as WSL2.

### Restructure to the manual's `ml/src/admet_ml/`

Consistency with the manual, and a properly installable package. Rejected because
the benefit is a benefit for a library, and this is not one. The cost is a rename
touching every path in existing commits and in `implementation.md`, for no change in
capability.

### Add a `pyproject.toml` to `training/` without moving it

The middle option: keep the path, gain installability. Not rejected so much as
deferred — it is cheap, and it becomes worth doing the moment anything needs to
import this code from outside the repository root. Recorded here so the option is not
forgotten.

## References

- Build Manual §2.1 (the WSL2 advice) and §4.1 (the `ml/src/admet_ml/` layout) — the
  two things this ADR departs from
- [`docs/00-machine-setup.md`](../00-machine-setup.md) — Windows install commands for
  every tool
- [`scripts/verify-env.sh`](../../scripts/verify-env.sh) and
  [`scripts/verify-env.ps1`](../../scripts/verify-env.ps1)
- `implementation.md` §8 — the `training/` layout this keeps
- [`.gitattributes`](../../.gitattributes) — the LF rule for shell scripts
