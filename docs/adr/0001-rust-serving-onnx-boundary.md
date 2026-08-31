# ADR-01: Serve inference from Rust, with ONNX as the only Python→Rust artefact

- **Status**: Accepted
- **Date**: 2026-08-12
- **Deciders**: tusharbeckham
- **Related**: TR-01, TR-02, TR-03, NFR-01, NFR-02, NFR-09, NFR-11, G6, G7, R1, R6

## Context

The system has two jobs with almost nothing in common.

**Training** needs the Python scientific stack. There is no serious alternative:
PyTorch, RDKit, scikit-learn and the whole experiment-tracking ecosystem live
there, and reimplementing gradient descent to avoid Python would be a different
project.

**Serving** needs predictable latency under batch load. The targets are NFR-01 —
p95 under 300 ms and p99 under 600 ms for a single molecule on a warm cache — and
NFR-02, a 10,000-molecule screen finishing inside 90 s on two vCPUs, which is to
say while a chemist is still at their desk. It also needs to run with no network
at all (G6) — pharmaceutical structures are the asset, and a meaningful fraction of
users will not put an unpublished structure into a hosted form at any price.

A single-language answer is available in both directions and neither is good.

Serving from Python means the request path carries the GIL and the garbage
collector. For a CPU-bound workload that is not fatal at low load; under a batch
screen it is exactly the wrong shape, because tail latency is what a user
experiences and the tail is where a collection pause lands. It also means the
"offline desktop build" ships a Python interpreter and a 400 MB dependency tree.

Training in Rust is not realistically possible in fifteen weeks.

So the system is bilingual, and the real decision is **where the seam goes and
what crosses it**.

## Decision

Python trains. Rust serves. The seam is a single ONNX file.

`models/model.onnx` is the **only** artefact that crosses from Python to Rust on
the serving path. No pickle, no TorchScript, no Python subprocess, no
`libtorch` FFI. `admet-infer` loads it through ONNX Runtime (`ort`) and the
request path contains no Python at all.

Two files accompany it and are also part of the contract, because a tensor of
floats is meaningless without them:

| File | Purpose |
|---|---|
| `models/feature_schema.json` | What each of the 33 feature columns means. Generated *from Rust* (`just schema`); Python reads it. |
| `models/model.onnx.sha256` | Which artefact produced a given prediction. Recorded in `model_versions.onnx_sha256`. |

## Consequences

### Positive

- **No GIL, no GC on the request path.** Latency is a function of arithmetic and
  memory, both of which can be measured and optimised. There is no third factor
  that occasionally adds 40 ms for reasons outside the process.
- **The offline desktop build is a wrapper, not a port.** `admet-core` +
  `admet-infer` + the `.onnx` is the whole inference stack, so Tauri links it
  directly. This is the payoff that arrives in Increment 5 and it is only
  available because of a decision made in week two.
- **The CLI is an honest benchmark harness.** `admet-cli bench` measures parse +
  featurise + infer with no HTTP, no pool, no cache. That number is the model's
  cost; an HTTP benchmark's number is the stack's.
- **A wrong prediction can be bisected in one command.** Run the same SMILES
  through the CLI and the API: if they agree, the model is the problem; if they
  disagree, the service is.
- **The artefact is inspectable.** `onnx.checker`, Netron, and any ONNX runtime
  can open it. A pickle can only be opened by the exact Python environment that
  wrote it, which is also a remote-code-execution primitive.

### Negative

- **The featuriser exists twice.** This is the single largest risk in the project
  (R3, TR-03). Python builds the 33-dim atom features for training; Rust builds
  them for serving. If they disagree about what column 17 means, nothing crashes
  — the model receives a plausible vector and returns a plausible number, and it
  is wrong. No unit test in either language catches it, because each is
  self-consistent.

  Mitigation, and it must be treated as load-bearing rather than nice to have:
  `feature_schema.json` is generated from the Rust `SCHEMA` and *asserted* by the
  Python featuriser; `just parity-fixture` dumps golden vectors that
  `crates/admet-infer/tests/onnx_parity.rs` re-derives in Rust and compares within
  1e-4; `feature_schema_version` is checked at server start-up and a mismatch is a
  **refusal to start**, not a warning.

- **ONNX constrains the model architecture.** Any operator without an ONNX export
  is off the table, which is what forced [ADR-03](0003-dense-adjacency-over-sparse-scatter.md).
  Choosing an architecture now means checking exportability *first*, which is a
  real restriction on the modelling work.
- **`ort` is pre-1.0.** The pin is exact (`=2.0.0-rc.13`) because rc-to-rc API
  churn has broken this build before. That is risk R6, and the cost is that
  dependency updates are a deliberate task rather than a `cargo update`.
- **Two toolchains to keep working.** Two lock files, two CI jobs, two ways for a
  machine to be misconfigured.

### Neutral

- Opset is pinned at export time (currently 17, verified round-tripping at 18).
  Whichever it is, it is recorded in the model card, because "it exported fine"
  is not reproducible information.
- Only the batch axis is dynamic in the exported graph. Everything else is static,
  which is what makes the artefact load in any runtime — see ADR-03.

## Alternatives considered

### Serve from Python with FastAPI

The path of least resistance, and it would have worked for a demo. Rejected
because it forfeits the two things that make this a platform rather than a
notebook: predictable tail latency under a batch screen, and an offline desktop
build that is not a 400 MB installer wrapping an interpreter. It would also have
made the entire Rust half of the project — and the systems-engineering content
that goes with it — vanish.

### TorchScript instead of ONNX

Closer to PyTorch, so fewer export surprises, and `tch-rs` exists. Rejected
because it drags `libtorch` into the Rust build: several hundred megabytes, a
platform-specific download step, and a linking story on Windows/MSVC that is
materially worse than ONNX Runtime's. It also does not open in Netron, and being
able to *look* at the graph mattered more than expected while debugging the export
spike.

### Rust-native training (`burn`, `candle`)

One language, one artefact, no parity risk — genuinely attractive on paper.
Rejected on ecosystem: no RDKit, so molecular featurisation would have to be
written from scratch before any modelling could start; and the published ADMET
baselines this project is measured against were all produced with the Python
stack, so reproducing them would become an unbounded research task rather than a
week's work.

### Keep PyTorch on the request path via PyO3

Embed the interpreter in the Rust process. Rejected quickly: it has every
downside of serving from Python (GIL, GC) plus the FFI complexity, and it makes
the desktop build worse rather than better.

## References

- `research.md` §4 — the dense/sparse consequence of this decision
- `method.md` §8 — the nine inference steps and which side owns each
- ONNX Runtime `ort` crate: <https://ort.pyke.io/>
- ONNX opset compatibility matrix: <https://onnxruntime.ai/docs/reference/compatibility.html>
- [ADR-03](0003-dense-adjacency-over-sparse-scatter.md) — what ONNX exportability forced on the architecture
- [ADR-02](0002-hexagonal-crate-split.md) — the crate layout that makes the desktop build a wrapper
