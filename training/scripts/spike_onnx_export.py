#!/usr/bin/env python3
"""
SPIKE: prove a dense-adjacency GIN exports to ONNX and round-trips.

Run this BEFORE writing any other code in this project.

Why this file exists
--------------------
The entire architecture rests on one unproven assumption: that the graph model
can leave Python as a portable ONNX artefact, so that Rust can serve it. If
that assumption is false, the Rust inference service is impossible and the
plan has to change.

PyTorch Geometric CANNOT do this. It aggregates neighbours with torch_scatter
segment reductions, which have no ONNX equivalent (see
pyg-team/pytorch_geometric#8415, pytorch/pytorch#65138). That is why this model
uses padded dense adjacency and torch.bmm instead.

This script needs no data, no training and no RDKit. It runs in seconds on
random graphs. It answers exactly one question: does the export work?

    python training/scripts/spike_onnx_export.py

Exit code 0 means the architecture is viable and you can proceed to Increment 1.
Exit code 1 means STOP and read the fallback note at the bottom of this file.
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

import numpy as np
import torch
from torch import nn

# --- Windows console encoding --------------------------------------------
#  Git Bash on Windows gives Python a cp1252 stdout. torch's dynamo exporter
#  prints a U+2705 into its own progress line, which raises
#  UnicodeEncodeError *inside* the exporter -- so the export appears to
#  "decline" and this script silently falls back to the deprecated
#  TorchScript path. The two paths produce different artefacts (47 KB vs
#  481 KB) at different opsets, which means the spike would validate a
#  different exporter depending on which terminal ran it. Force UTF-8 so the
#  route is a property of the toolchain, not of the console.
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")  # type: ignore[union-attr]
    except (AttributeError, OSError):  # pragma: no cover -- non-tty stream
        pass

# --- the contract from method.md -----------------------------------------
MAX_ATOMS = 128
N_FEATURES = 33
N_ENDPOINTS = 12
HIDDEN = 128
OPSET = 17
TOLERANCE = 1e-5


# =========================================================================
#  Model
# =========================================================================
class DenseGINLayer(nn.Module):
    """One GIN message-passing round over a dense adjacency matrix.

    The whole point: aggregation is a single batched matmul, which ONNX
    exports cleanly. There is no scatter, no index_select over a dynamic
    axis, and no data-dependent control flow.
    """

    def __init__(self, d_in: int, d_out: int) -> None:
        super().__init__()
        self.mlp = nn.Sequential(
            nn.Linear(d_in, d_out),
            nn.ReLU(),
            nn.Linear(d_out, d_out),
        )

    def forward(self, x: torch.Tensor, adj: torch.Tensor) -> torch.Tensor:
        # x   [B, N, d_in]
        # adj [B, N, N]  already symmetric-normalised with self-loops
        return self.mlp(torch.bmm(adj, x))


class DenseGIN(nn.Module):
    def __init__(self) -> None:
        super().__init__()
        self.l1 = DenseGINLayer(N_FEATURES, HIDDEN)
        self.l2 = DenseGINLayer(HIDDEN, HIDDEN)
        self.l3 = DenseGINLayer(HIDDEN, HIDDEN)
        self.bn1 = nn.BatchNorm1d(HIDDEN)
        self.bn2 = nn.BatchNorm1d(HIDDEN)
        self.bn3 = nn.BatchNorm1d(HIDDEN)
        self.head = nn.Sequential(
            nn.Linear(HIDDEN * 2, HIDDEN),
            nn.ReLU(),
            nn.Dropout(0.3),
            nn.Linear(HIDDEN, N_ENDPOINTS),
        )

    @staticmethod
    def _norm(bn: nn.BatchNorm1d, h: torch.Tensor) -> torch.Tensor:
        # BatchNorm1d over [B, N, C] wants channels in dim 1 -> [B, C, N].
        return bn(h.transpose(1, 2)).transpose(1, 2)

    def forward(
        self, x: torch.Tensor, adj: torch.Tensor, mask: torch.Tensor
    ) -> torch.Tensor:
        h = torch.relu(self._norm(self.bn1, self.l1(x, adj)))
        h = torch.relu(self._norm(self.bn2, self.l2(h, adj)))
        h = torch.relu(self._norm(self.bn3, self.l3(h, adj)))

        m = mask.unsqueeze(-1)  # [B, N, 1]
        h = h * m  # kill the padding

        # Masked mean: divide by the real atom count, not by MAX_ATOMS.
        # Getting this wrong shrinks every embedding by N/128 and the model
        # still trains -- just badly, and silently.
        n_real = m.sum(dim=1).clamp(min=1.0)  # [B, 1]
        mean_pool = h.sum(dim=1) / n_real  # [B, C]

        # Masked max: padding must not win the max.
        max_pool = h.masked_fill(m == 0, -1e9).max(dim=1).values

        return self.head(torch.cat([mean_pool, max_pool], dim=-1))


# =========================================================================
#  Synthetic graphs (no RDKit needed for a spike)
# =========================================================================
def random_batch(batch_size: int, seed: int = 0):
    """Build a batch of plausible molecule-shaped graphs.

    Connected, undirected, self-looped, symmetric-normalised -- the same
    shape contract the real featuriser must produce.
    """
    rng = np.random.default_rng(seed)
    x = np.zeros((batch_size, MAX_ATOMS, N_FEATURES), dtype=np.float32)
    adj = np.zeros((batch_size, MAX_ATOMS, MAX_ATOMS), dtype=np.float32)
    mask = np.zeros((batch_size, MAX_ATOMS), dtype=np.float32)

    for b in range(batch_size):
        n = int(rng.integers(5, 61))  # 5..60 heavy atoms
        mask[b, :n] = 1.0

        for i in range(n):
            x[b, i, int(rng.integers(0, 10))] = 1.0  # element block
            x[b, i, 10 + int(rng.integers(0, 6))] = 1.0  # degree block
            x[b, i, 31] = float(rng.integers(0, 2))  # aromatic flag

        # spanning tree guarantees connectivity, plus a few ring closures
        a = np.eye(n, dtype=np.float32)  # self-loops
        for i in range(1, n):
            j = int(rng.integers(0, i))
            a[i, j] = a[j, i] = 1.0
        for _ in range(max(1, n // 10)):
            i, j = (int(v) for v in rng.integers(0, n, size=2))
            if i != j:
                a[i, j] = a[j, i] = 1.0

        # D^-1/2 (A + I) D^-1/2
        deg = a.sum(axis=1)
        dinv = 1.0 / np.sqrt(np.maximum(deg, 1e-12))
        adj[b, :n, :n] = a * dinv[:, None] * dinv[None, :]

    return x, adj, mask


# =========================================================================
#  Checks
# =========================================================================
def main() -> int:
    torch.manual_seed(0)
    results: list[tuple[str, bool, str]] = []

    print("=" * 68)
    print("  ADMETriage -- ONNX export spike")
    print("=" * 68)
    print(f"  torch      {torch.__version__}")
    try:
        import onnx
        import onnxruntime as ort

        print(f"  onnx       {onnx.__version__}")
        print(f"  onnxruntime {ort.__version__}")
    except ImportError as exc:
        print(f"\n  MISSING DEPENDENCY: {exc}")
        print("  pip install onnx onnxruntime")
        return 1
    print(f"  opset      {OPSET}")
    print(
        f"  shapes     x[B,{MAX_ATOMS},{N_FEATURES}]  "
        f"adj[B,{MAX_ATOMS},{MAX_ATOMS}]  mask[B,{MAX_ATOMS}]"
    )
    print()

    model = DenseGIN()
    model.eval()  # CRITICAL. Exporting in train mode bakes batch statistics
    # into the graph and the served model then disagrees with
    # the trained one.

    n_params = sum(p.numel() for p in model.parameters())
    print(f"  [1/5] model built ....................... {n_params:,} params")

    # ---- forward pass in PyTorch ----------------------------------------
    x, adj, mask = random_batch(4, seed=1)
    tx = torch.from_numpy(x)
    tadj = torch.from_numpy(adj)
    tmask = torch.from_numpy(mask)

    with torch.no_grad():
        torch_out = model(tx, tadj, tmask).numpy()
    print(f"  [2/5] pytorch forward ................... {torch_out.shape}")

    tmpdir = Path(tempfile.mkdtemp(prefix="admet_spike_"))
    onnx_path = tmpdir / "spike_tiny_gin.onnx"

    # ---- export ---------------------------------------------------------
    #  torch.onnx.export changed materially between 2.6 and 2.11:
    #    2.6   do_constant_folding deprecated -- it is always enabled now
    #    2.9   dynamo flipped to True by DEFAULT
    #    2.10  fallback defaults to False -- no silent retry on the old path
    #    2.11  fallback removed entirely
    #
    #  dynamic_axes is the LEGACY (dynamo=False) vocabulary; dynamic_shapes
    #  is the torch.export vocabulary. PyTorch will auto-convert the former,
    #  but that conversion has documented corner cases
    #  (pytorch/pytorch#150940, #150544), so we state the modern form
    #  explicitly rather than depending on a best-effort translation.
    #
    #  Passing ONE Dim object to all three inputs asserts that they share a
    #  single batch size. Three independently named axes would not: the
    #  exporter would accept a graph where x has 8 rows and adj has 5.
    def _export_dynamo() -> None:
        from torch.export import Dim

        batch = Dim("batch")
        torch.onnx.export(
            model,
            (tx, tadj, tmask),
            str(onnx_path),
            opset_version=OPSET,
            input_names=["x", "adj", "mask"],
            output_names=["predictions"],
            dynamic_shapes=({0: batch}, {0: batch}, {0: batch}),
            dynamo=True,
            #  DEF-01: dynamo=True defaults to external_data=True, which writes
            #  the weights to a SEPARATE `<name>.onnx.data` sidecar. Copying only
            #  the .onnx into fixtures/ therefore produced a file that no runtime
            #  can load ("External data path does not exist"). At 47 KB the
            #  sidecar buys nothing, so the artefact is forced self-contained --
            #  a committed fixture must be a single file to be useful to CI.
            external_data=False,
        )

    def _export_legacy() -> None:
        torch.onnx.export(
            model,
            (tx, tadj, tmask),
            str(onnx_path),
            opset_version=OPSET,
            input_names=["x", "adj", "mask"],
            output_names=["predictions"],
            dynamic_axes={
                "x": {0: "batch"},
                "adj": {0: "batch"},
                "mask": {0: "batch"},
                "predictions": {0: "batch"},
            },
            dynamo=False,
        )

    try:
        try:
            _export_dynamo()
            route = "torch.export"
        except Exception as modern_exc:  # noqa: BLE001
            print(
                f"        dynamo exporter declined: "
                f"{type(modern_exc).__name__}: {modern_exc}"
            )
            print("        falling back to the legacy TorchScript exporter")
            _export_legacy()
            route = "legacy TorchScript"
        size_kb = onnx_path.stat().st_size / 1024
        print(
            f"  [3/5] onnx export ....................... {size_kb:,.0f} KB  [{route}]"
        )
        results.append(("export", True, f"{size_kb:,.0f} KB / {route}"))
    except Exception as exc:  # noqa: BLE001
        print(f"  [3/5] onnx export ....................... FAILED\n\n{exc}")
        results.append(("export", False, str(exc)[:200]))
        _summary(results)
        return 1

    # ---- structural validation ------------------------------------------
    #  Read the opset back off the artefact rather than trusting the number
    #  we asked for. torch >= 2.9's dynamo exporter silently RAISES a request
    #  for 17 to 18 ("we have no implementations below 18") and then fails to
    #  down-convert, so `opset_version=17` can produce an opset-18 graph. A
    #  script that prints the requested number and never checks the delivered
    #  one is how a repository ends up with two different opsets on record.
    actual_opset: int | None = None
    try:
        loaded = onnx.load(str(onnx_path))
        onnx.checker.check_model(loaded)
        actual_opset = next(
            (o.version for o in loaded.opset_import if o.domain in ("", "ai.onnx")),
            None,
        )
        print(
            f"  [4/5] onnx.checker ...................... valid "
            f"(ir {loaded.ir_version}, opset {actual_opset})"
        )
        results.append(("checker", True, f"valid graph / opset {actual_opset}"))
    except Exception as exc:  # noqa: BLE001
        print(f"  [4/5] onnx.checker ...................... FAILED\n\n{exc}")
        results.append(("checker", False, str(exc)[:200]))

    if actual_opset is not None and actual_opset != OPSET:
        print(
            f"\n  [!] OPSET MISMATCH: requested {OPSET}, artefact is "
            f"{actual_opset}.\n"
            f"      The exporter refused to down-convert. Either set "
            f"OPSET = {actual_opset}\n"
            f"      here and amend TR-01, or export through a toolchain that "
            f"honours {OPSET}.\n"
            f"      This is not a spike failure -- the round trip works. It "
            f"is a\n"
            f"      documentation defect, and it is open question Q-1 in "
            f"docs/01-srs.md.\n"
        )
        results.append(
            (
                "opset",
                True,
                f"{actual_opset} DELIVERED, {OPSET} requested -- Q-1 UNRESOLVED",
            )
        )

    # ---- numerical parity across batch sizes ----------------------------
    print("  [5/5] runtime parity")
    sess = ort.InferenceSession(str(onnx_path), providers=["CPUExecutionProvider"])

    ok_all = True
    for bs in (1, 7, 64):
        bx, badj, bmask = random_batch(bs, seed=100 + bs)
        with torch.no_grad():
            expect = model(
                torch.from_numpy(bx),
                torch.from_numpy(badj),
                torch.from_numpy(bmask),
            ).numpy()
        got = sess.run(["predictions"], {"x": bx, "adj": badj, "mask": bmask})[0]

        if got.shape != expect.shape:
            print(
                f"        batch {bs:<3} shape mismatch "
                f"{got.shape} vs {expect.shape}   FAIL"
            )
            ok_all = False
            continue

        diff = float(np.max(np.abs(got - expect)))
        verdict = "ok" if diff < TOLERANCE else "FAIL"
        if diff >= TOLERANCE:
            ok_all = False
        print(f"        batch {bs:<3} max abs diff {diff:.3e}   {verdict}")

    results.append(("parity", ok_all, f"tolerance {TOLERANCE:.0e}"))
    return _summary(results, onnx_path)


def _summary(results, onnx_path: Path | None = None) -> int:
    passed = all(ok for _, ok, _ in results)
    print()
    print("=" * 68)
    for name, ok, note in results:
        print(f"  {'PASS' if ok else 'FAIL'}  {name:<10} {note}")
    print("=" * 68)

    if passed:
        print("""
  SPIKE PASSED.

  The dense-adjacency GIN exports to ONNX and round-trips numerically
  across dynamic batch sizes. The Rust inference path is viable.

  Next steps:
    1. Publish the artefact with `--publish` (copies it to
       fixtures/spike_tiny_gin.onnx) and commit it, so CI can prove the
       round-trip without a training run.
    2. Load it from Rust with `ort` and assert the same outputs. That
       closes the loop end to end before any real model exists.
    3. Proceed to Increment 1 (data pipeline + real training).
""")
        if onnx_path is not None:
            print(f"  artefact: {onnx_path}\n")
            #  Publishing is opt-in: a spike that silently overwrites a
            #  committed fixture would make an accidental model change look
            #  like a passing test.
            if "--publish" in sys.argv:
                import shutil

                import onnxruntime as _ort

                dest = (
                    Path(__file__).resolve().parents[2]
                    / "fixtures"
                    / "spike_tiny_gin.onnx"
                )
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(onnx_path, dest)
                #  Assert the published copy actually loads standalone. The
                #  previous fixture did not (DEF-01) and nothing caught it.
                _ort.InferenceSession(str(dest), providers=["CPUExecutionProvider"])
                print(
                    f"  published: {dest} "
                    f"({dest.stat().st_size / 1024:.0f} KB, loads standalone)\n"
                )
        return 0

    print("""
  SPIKE FAILED -- STOP AND READ.

  Do not start writing the Rust crates. The plan needs one change first.

  Diagnose in this order:
    * Did you reintroduce torch-geometric / torch-scatter? Those ops cannot
      export. This model must use torch.bmm on a dense adjacency.
    * Is any tensor shape dependent on the input data? Only the BATCH axis
      may be dynamic; the atom axis must be fixed at 128.
    * Try opset 18 or 19. Occasionally an op gains support in a later opset.
    * Was model.eval() called before export? BatchNorm in train mode can
      emit ops that fail to fold.

  If it still fails, the fallback is a thin Python inference sidecar behind
  the same HTTP contract: admet-infer keeps its trait boundary but calls out
  over a local socket instead of running the session in-process. The Rust
  crates keep their structure, the API contract is unchanged, and the
  latency target gets revised honestly from p95 300 ms to something
  achievable. That is a small adjustment in week 1 and a rewrite in week 8,
  which is the entire reason this script runs first.
""")
    return 1


if __name__ == "__main__":
    sys.exit(main())
