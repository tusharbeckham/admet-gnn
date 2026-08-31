# Increment 0 — scaffold verification evidence

Captured on **2026-08-31**, on the Owner's machine (Windows 11, native + Git Bash
per ADR-07), the first time the Rust half of this repository was ever compiled.

Context: the scaffold for manual Parts One and Two (chapters 2–5) was written
before a Rust toolchain existed on this machine. `rustc`, `cargo`, `just`, `pnpm`
and `typst` were all missing, and there was no MSVC linker, so five crates and a
Python↔Rust parity test had been committed without ever being built or run. These
files record what happened when they finally were.

| File | What it shows | Command |
|---|---|---|
| `verify-env.txt` | Every tool the project needs, resolved. | `bash scripts/verify-env.sh` |
| `nextest.txt` | 119 Rust tests passing, 17 `#[ignore]`d placeholders skipped. | `cargo nextest run --workspace` |
| `pytest.txt` | 11 Python tests passing over `scaffold_split` and `clean`. | `python -m pytest training/tests -q` |
| `audit-rsa-not-in-build-graph.txt` | `rsa` (RUSTSEC-2023-0071) is absent from the build graph — the evidence behind the single entry in `.cargo/audit.toml`. | `cargo tree -i rsa --target all` |

## The number that matters

`tests/onnx_parity.rs::rust_matches_python_on_the_committed_model` passed. That
test compares Rust's `ort` output against a golden fixture produced by Python's
`onnxruntime` and agrees within the manifest's tolerance across batch sizes 1 and
3. It closes the Python↔Rust loop that ADR-01 is built on, and until this run it
had never executed.

Getting there took eight defects, logged as **DEF-01 … DEF-08** in
[`../../03-test-plan.md`](../../03-test-plan.md) §10.2. Three were **S1** — a
wrong number that looks right:

- the committed ONNX fixture could not be loaded at all (missing external-data
  sidecar);
- `MolGraph::default()` violated the CSR invariant it validates itself against;
- one fatal endpoint scored **0.119** instead of ~0, so the triage score did not
  actually disqualify a likely hERG blocker.

## Honesty notes

- These are **scaffold** numbers. No model has been trained, no TDC data has been
  downloaded, and no endpoint metric exists yet. Nothing here is evidence about
  predictive performance.
- 17 tests are skipped by design: they are `#[ignore]`d placeholders naming work
  that Increments 1–5 will fill in. A skipped test is not a passing test.
- `opset` remains open question **Q-1**: the exporter delivers opset 18 where the
  spike requests 17. The round-trip works; the documentation disagrees with the
  artefact, and that is recorded rather than quietly reconciled.
- Postgres was reachable (`just db-up`) and `just db-migrate` is a clean no-op —
  there are no migrations until Increment 2, so this proves connectivity only.
