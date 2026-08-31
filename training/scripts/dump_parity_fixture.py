#!/usr/bin/env python3
"""Dump a golden input/output fixture so Rust can prove Python-Rust parity.

WHY THIS EXISTS
---------------
spike_onnx_export.py proved one boundary: PyTorch -> ONNX. It did not prove
the boundary that actually matters in production, which is

    ONNX artefact  ->  ONNX Runtime *inside Rust*

Those are different code paths. Python's onnxruntime and Rust's `ort` both
wrap the same C++ library, so they *should* agree bit for bit -- but "should"
is how you end up debugging a 0.03 AUROC gap in week nine.

This script freezes the question into data. It runs the committed ONNX model
through Python's onnxruntime, then writes the inputs AND the outputs to disk
as raw little-endian float32. The Rust test loads the same model, feeds the
same bytes, and must reproduce the same numbers.

Crucially, the expected outputs come from the ONNX MODEL, not from PyTorch.
That is deliberate. Torch-vs-ONNX is already covered by the spike; mixing the
two comparisons into one fixture would mean a failure could not tell you
which boundary broke.

USAGE
-----
    python training/scripts/dump_parity_fixture.py

Writes fixtures/parity/. Commit the whole directory -- it is a few hundred KB
of mostly zeros, so git compresses it to almost nothing.

Re-run this ONLY when fixtures/spike_tiny_gin.onnx changes. If you re-run it
for any other reason and the Rust test then passes, you have proved nothing:
you just moved the goalposts to wherever the ball landed.
"""

from __future__ import annotations

import json
import sys
from datetime import datetime, timezone
from pathlib import Path

import numpy as np

# --- the contract from method.md -----------------------------------------
MAX_ATOMS = 128
N_FEATURES = 33
N_ENDPOINTS = 12

# Batch sizes to freeze. Keep this SMALL: an adjacency matrix is 128x128
# floats, so every extra batch unit costs ~83 KB of fixture. Batch 1 exercises
# the degenerate case, batch 3 exercises the dynamic axis. Batch 64 is already
# covered inside the Python spike and would add 5 MB here for no new signal.
BATCHES = (1, 3)

# Fixed seed, and it must never change casually -- see the docstring warning.
SEED = 20260825

# Looser than the spike's 1e-5 on purpose. This is the END-TO-END budget from
# requirements.md TR-04, and it has to absorb two runtimes, two allocators and
# two threading strategies. In practice you should see ~1e-9.
TOLERANCE = 1e-4

REPO_ROOT = Path(__file__).resolve().parents[2]
MODEL = REPO_ROOT / "fixtures" / "spike_tiny_gin.onnx"
OUT_DIR = REPO_ROOT / "fixtures" / "parity"


# =========================================================================
#  Synthetic graphs -- VERBATIM copy of random_batch() in
#  spike_onnx_export.py. Kept duplicated rather than imported: this script
#  must keep producing the same bytes even if the spike is later deleted,
#  and a fixture generator that silently follows someone else's refactor is
#  a fixture generator you cannot trust.
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


def write_f32(path: Path, arr: np.ndarray) -> int:
    """Write a contiguous little-endian float32 blob. Returns bytes written.

    '<f4' is explicit rather than relying on the platform being
    little-endian. It is, on every machine this project targets, but a
    fixture format that depends on the host byte order is not a format.
    """
    blob = np.ascontiguousarray(arr, dtype="<f4").tobytes()
    path.write_bytes(blob)
    return len(blob)


def main() -> int:
    print("=" * 68)
    print("  ADMETriage -- golden parity fixture")
    print("=" * 68)

    if not MODEL.exists():
        print(f"\n  MODEL NOT FOUND: {MODEL}")
        print("  Run training/scripts/spike_onnx_export.py first, then copy")
        print("  its artefact to fixtures/spike_tiny_gin.onnx.")
        return 1

    try:
        import onnxruntime as ort
    except ImportError as exc:
        print(f"\n  MISSING DEPENDENCY: {exc}")
        return 1

    sess = ort.InferenceSession(str(MODEL), providers=["CPUExecutionProvider"])

    in_meta = sess.get_inputs()
    out_meta = sess.get_outputs()
    print(f"  onnxruntime {ort.__version__}")
    print(f"  model       {MODEL.relative_to(REPO_ROOT).as_posix()}")
    print(f"  inputs      {[m.name for m in in_meta]}")
    print(f"  outputs     {[m.name for m in out_meta]}")
    print()

    # The Rust side feeds inputs POSITIONALLY, in graph order. If the graph
    # ever gains, loses or reorders an input, that assumption breaks silently
    # and the parity numbers become meaningless. Fail loudly instead.
    if len(in_meta) != 3:
        print(f"  UNEXPECTED: model has {len(in_meta)} inputs, expected 3")
        print("  The fixture format assumes exactly (x, adj, mask).")
        return 1
    if len(out_meta) != 1:
        print(f"  UNEXPECTED: model has {len(out_meta)} outputs, expected 1")
        return 1

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    input_names = [m.name for m in in_meta]
    output_name = out_meta[0].name

    cases = []
    total = 0
    for bs in BATCHES:
        x, adj, mask = random_batch(bs, seed=SEED + bs)
        feed = {input_names[0]: x, input_names[1]: adj, input_names[2]: mask}
        (pred,) = sess.run([output_name], feed)
        pred = np.ascontiguousarray(pred, dtype=np.float32)

        if pred.shape != (bs, N_ENDPOINTS):
            print(
                f"  UNEXPECTED output shape {pred.shape}, expected {(bs, N_ENDPOINTS)}"
            )
            return 1

        files = []
        for arr, logical in ((x, "x"), (adj, "adj"), (mask, "mask")):
            name = f"b{bs}_{logical}.f32"
            total += write_f32(OUT_DIR / name, arr)
            files.append(name)

        exp_name = f"b{bs}_expected.f32"
        total += write_f32(OUT_DIR / exp_name, pred)

        cases.append(
            {
                "batch": bs,
                "shapes": [list(x.shape), list(adj.shape), list(mask.shape)],
                "inputs": files,
                "expected": exp_name,
                "expected_shape": list(pred.shape),
            }
        )

        span = f"{float(pred.min()):+.6f} .. {float(pred.max()):+.6f}"
        print(f"  batch {bs:<3} logits {span}")

    manifest = {
        "_comment": (
            "Generated by training/scripts/dump_parity_fixture.py. Do not "
            "edit by hand, and do not regenerate to make a failing test "
            "pass -- regenerate only when the ONNX model itself changes."
        ),
        "generated": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "model": MODEL.relative_to(REPO_ROOT).as_posix(),
        "onnxruntime": ort.__version__,
        "dtype": "float32-le",
        "tolerance": TOLERANCE,
        "seed": SEED,
        "input_names": input_names,
        "output_name": output_name,
        "max_atoms": MAX_ATOMS,
        "n_features": N_FEATURES,
        "n_endpoints": N_ENDPOINTS,
        "cases": cases,
    }
    (OUT_DIR / "manifest.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )

    print()
    print(f"  wrote {len(cases) * 4 + 1} files, {total / 1024:.0f} KB")
    print(f"  -> {OUT_DIR.relative_to(REPO_ROOT).as_posix()}/")
    print()
    print("  Next:  cargo test -p admet-infer")
    return 0


if __name__ == "__main__":
    sys.exit(main())
