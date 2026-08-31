# ADR-03: Use padded dense adjacency and `torch.bmm`, not PyTorch Geometric scatter

- **Status**: Accepted
- **Date**: 2026-08-14
- **Deciders**: tusharbeckham
- **Related**: TR-01, TR-02, TR-03, FR-04, NFR-01, NFR-02, R1, R3

## Context

[ADR-01](0001-rust-serving-onnx-boundary.md) commits the serving path to Rust,
which means the model must leave Python as an ONNX graph. Everything below follows
from taking that seriously.

The obvious way to build a molecular GNN is PyTorch Geometric. It is the standard
library, the tutorials use it, and the original plan for this project assumed it.
PyG batches molecules by concatenating them into one large disconnected graph and
aggregates neighbours with `torch_scatter` segment reductions
(`scatter_add`, `scatter_max`).

**Those operators have no ONNX equivalent.** The export either fails outright or
emits a graph that no runtime will load. This is long-standing and well
documented, not a version bug that a `pip install -U` fixes:

- `pyg-team/pytorch_geometric` issue #8415
- `pytorch/pytorch` issue #65138
- Chemprop issue #1335 — the same wall, hit by the reference D-MPNN implementation

Discovering this in week nine, with a trained PyG model and a Rust service that
cannot load it, is the single most expensive failure available to this project.
So it was tested in week two, as a deliberate spike, before any modelling.

Three ways out, none free:

| Option | Consequence |
|---|---|
| Keep PyG, serve from Python | Abandons the Rust API and the whole latency argument (reverts ADR-01) |
| Keep PyG, write custom ONNX operators for the scatter ops | Weeks of fragile work; no realistic path in a semester |
| **Padded dense adjacency, `torch.bmm`** | **Wasted arithmetic on padding; everything else works** |

## Decision

No PyTorch Geometric. No `torch_scatter`. Message passing is a batched dense
matrix multiply.

Every molecule is padded to a fixed **128 heavy atoms** and represented by three
tensors:

| Tensor | Shape | Contents |
|---|---|---|
| `x` | `[B, 128, 33]` | atom features, zero rows for padding |
| `adj` | `[B, 128, 128]` | adjacency, normalised, zero rows/cols for padding |
| `mask` | `[B, 128]` | 1.0 for a real atom, 0.0 for padding |

Neighbour aggregation is `torch.bmm(adj, x)`. Readout is a masked sum/mean over
the atom axis, so padding contributes nothing.

**Only the batch axis is dynamic in the exported graph.** Atom count and feature
count are static, which is what makes the artefact load unmodified in any ONNX
runtime.

**Molecules above 128 heavy atoms are rejected, never truncated.** A truncated
molecule is a different molecule, and a prediction for a different molecule
presented as a prediction for this one is the worst failure mode this system can
have — it is confidently wrong and nothing in the output signals it. The rejection
is a typed 422 with the atom count in the message (`ApiError::TooLarge`).

## Consequences

### Positive

- **The export works, and the shapes are static.** Verified round-tripping across
  batch sizes 1, 7 and 64 by `training/scripts/spike_onnx_export.py`, with the
  tiny artefact committed at `fixtures/spike_tiny_gin.onnx` so CI can prove it
  without a training run.
- **The Rust side is simple.** Three tensors of known shape. No CSR index
  arithmetic on the request path, no segment-offset bookkeeping, no chance of an
  off-by-one in a batch offset producing a silently wrong aggregation.
- **Padding is uniform, so batching is trivial.** Every molecule is the same shape,
  which means micro-batching (NFR-02, TR-08) is a stack, not a ragged-tensor problem.
- **Dependency tree shrinks substantially.** No `torch-geometric`, no
  `torch-scatter`, no `torch-sparse` — three packages that need compilation against
  a specific torch version and are the most common cause of an environment that
  cannot be recreated six months later.

### Negative

- **Wasted arithmetic, quantifiably.** For a 30-atom molecule padded to 128, dense
  aggregation is roughly **2.1 million** multiply-adds per layer against about
  **17 thousand** for sparse — a factor of ~125.

  This sounds fatal and is not, for reasons worth stating rather than asserting: at
  128×128 the matrices are small enough that a single BLAS call saturates a modern
  CPU core, and the constant factor of a dense `bmm` on contiguous memory is far
  better than scattered indirect access. The measured cost is a few milliseconds
  per molecule. **That figure is a target until `just bench` prints it** — the
  arithmetic count above is exact, the conclusion about wall time is not, and the
  distinction matters.

- **Memory scales with the cap, not the molecule.** `adj` at `[64, 128, 128]`
  float32 is 4 MB per batch regardless of whether the molecules have 12 atoms or
  120. Raising the cap to 256 quadruples it.
- **The 128 cap is a real product limitation.** Peptides, large macrocycles and
  most PROTACs exceed it. `just data-profile` reports the excluded fraction per
  endpoint, and that number belongs in the report as a limitation rather than
  being quietly absent. Raising the cap means retraining, because the padded shape
  is baked into the exported graph.
- **No access to PyG's model zoo.** GAT, GraphSAGE, GIN variants and the attention
  layers all have to be written against dense tensors. Each is a few dozen lines,
  but it is a few dozen lines that could have been an import.

### Neutral

- The normalisation applied to `adj` (symmetric $D^{-1/2}AD^{-1/2}$ versus a plain
  sum with a self-loop) is a modelling choice, not an architectural one, and is
  recorded in `method.md` rather than here.
- A dense adjacency makes an attention-based explainability method *easier*, not
  harder: the per-atom contribution is already a slice of a dense tensor, which is
  what FR-19's per-atom attribution needs.

## Alternatives considered

### Keep PyG and serve from Python

Would have worked, and would have been faster to build. Rejected because it
reverts [ADR-01](0001-rust-serving-onnx-boundary.md) entirely: no predictable tail
latency, no offline desktop build, and the Rust half of the project disappears.

### Custom ONNX operators for `scatter_add`

Technically the "right" fix — implement the operator in the runtime and register
it. Rejected on schedule and fragility: it requires a C++ custom-op build against
ONNX Runtime, a matching implementation in `ort`, and it breaks every time either
side updates. `ort` is already pinned at an rc (risk R6); adding a custom-op
dependency on top of that is a project of its own.

### Sparse CSR message passing implemented by hand in both languages

Write the scatter yourself, in PyTorch using only exportable ops, and again in
Rust. Rejected because it doubles the surface of the parity risk (R3) at exactly
the point where it is most dangerous: an off-by-one in a row-offset array produces
a graph with slightly wrong edges, which trains and serves and is wrong. Dense
`bmm` has no index arithmetic to get wrong.

### Truncate large molecules instead of rejecting them

Considered and rejected on correctness, not convenience. Truncation produces a
prediction for a molecule the user did not submit, with no signal in the output
that it happened. Rejection loses a data point; truncation manufactures a wrong
answer. Explicitly tested (`oversized_molecules_are_422_not_400`).

### A larger cap, e.g. 256 atoms

Covers more chemistry at 4× the adjacency memory and roughly 4× the aggregation
cost. Rejected for now because `just data-profile` shows the twelve TDC endpoints
are overwhelmingly small molecules — the excluded fraction at 128 is small enough
that the cost is not worth paying. This is a number the profile step produces, and
if it comes back higher than expected the decision should be revisited rather than
defended.

## References

- `research.md` §4 — the full argument, including the FLOP comparison
- `method.md` §3b — the dense tensor shape table
- `training/scripts/spike_onnx_export.py` — the spike that verified this
- `fixtures/spike_tiny_gin.onnx` — the committed artefact CI checks against
- [ADR-01](0001-rust-serving-onnx-boundary.md) — the constraint that forced this
