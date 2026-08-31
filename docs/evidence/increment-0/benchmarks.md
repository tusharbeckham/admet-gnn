# Increment 0 — Criterion baselines for `admet-core`

Captured **2026-08-31** on the Owner's machine, `cargo bench -p admet-core --bench core`,
release profile with `lto = "thin"` and `codegen-units = 1`.

| Benchmark | Median | What it is |
|---|---|---|
| `fingerprint/can_exceed` | **2.04 ns** | The cheap pre-filter that rejects a Tanimoto comparison before computing it. |
| `fingerprint/popcount` | **39.4 ns** | Population count over a fingerprint. |
| `validate_input/aspirin` | **42.8 ns** | Input validation for one small molecule. |
| `fingerprint/tanimoto` | **72.0 ns** | One Tanimoto similarity. |
| `applicability_domain/assess_*` | **7.1 µs → 2.63 ms** | Domain assessment, scaling across the four sizes in `benches/core.rs`. |
| `triage/top_100_of_1000` | **97.0 µs** | Rank 1,000 scored candidates, keep 100. |
| `triage/top_100_of_10000` | **663 µs** | Same at 10,000. |
| `triage/top_100_of_50000` | **2.56 ms** | Same at the FR-14 batch ceiling of 50,000 rows. |

## What these do and do not tell you

**They are not NFR-01 or NFR-02 results.** NFR-01 is single-prediction p95 under
300 ms and NFR-02 is a 10,000-molecule batch in under 90 s on 2 vCPU. Both include
featurisation, ONNX inference, HTTP and cache — none of which is measured here, and
two of which do not exist yet. Quoting `2.56 ms` as evidence for a latency
requirement would be dishonest by three orders of magnitude.

What they *do* establish: **ranking is not the bottleneck and will not become one.**
Sorting the full FR-14 ceiling of 50,000 candidates costs 2.56 ms, roughly 0.85% of
the 300 ms single-prediction budget, and it scales close to linearly from 1,000
(97 µs → 663 µs → 2.56 ms for 10×, 50× the work). So when the batch path misses its
target later, this is not where to look — which is worth knowing in advance, because
sorting is a tempting thing to optimise and would have been wasted effort.

## The LTO question stays open, on purpose

`Cargo.toml` keeps `lto = "thin"` against the manual's `"fat"`. Chapter 24.2 lists
LTO plus `codegen-units = 1` as optimisation #4, to be **measured**. These numbers
are the "before" half of that comparison. Nothing has been concluded yet, and the
`--save-baseline scaffold` baseline is saved so a future `--baseline scaffold` run
produces a real delta rather than a recollection.

## Caveat on the numbers themselves

Measured on a developer laptop under ordinary load, not a quiet 2-vCPU box. The
confidence intervals are wide — `applicability_domain` at the largest size spans
757 µs to 1.04 ms — and the same benchmarks moved 15–40% between two consecutive
runs (`validate_input/aspirin` was 34.6 ns then 42.8 ns). Treat them as an order of
magnitude, not a measurement to defend to three significant figures. The report's
performance chapter needs a quiet machine.
