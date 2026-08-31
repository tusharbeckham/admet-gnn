"""
Exploratory profile of the twelve downloaded endpoints.

    .venv/Scripts/python.exe training/data/profile.py
    # or:
    just data-profile

Writes `results/data_profile.json` and prints a table.

Run this BEFORE modelling anything. Surprises here are cheap; the same
surprises found in week nine are not. This is step 1 of Increment 1
(manual ch. 18.2) and its output is a one-page exploratory summary that
belongs in the report's data chapter.

Specifically, it answers the four questions that change downstream decisions:

  1. How many rows actually parse?      -> if a dataset loses 5% to unparseable
                                           SMILES, that is a finding, not noise.
  2. What is the class balance?          -> drives `pos_weight` per endpoint, and
                                           tells you which endpoints cannot be
                                           scored with accuracy.
  3. How large are the molecules?        -> the model caps at 128 heavy atoms.
                                           If more than ~1% exceed it, the cap
                                           itself needs revisiting.
  4. What is the target distribution?    -> long tails mean Huber loss, not MSE.

Unlike `download_tdc.py`, this file DOES use RDKit, and so must run in the main
`.venv` (rdkit 2026.3.5). That split is the whole point -- see the banner in
`download_tdc.py`.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pandas as pd
from download_tdc import ENDPOINTS, LABEL_COL, RAW_DIR, SMILES_COL
from rdkit import Chem, RDLogger

# RDKit is chatty about every unparseable SMILES. We count them ourselves and
# report the total, which is more useful than several hundred stderr lines.
RDLogger.DisableLog("rdApp.*")

REPO_ROOT = Path(__file__).resolve().parents[2]
RESULTS_DIR = REPO_ROOT / "results"

# The hard cap baked into the exported ONNX graph. Molecules above this are
# REJECTED, never truncated -- a truncated molecule is a different molecule, and
# confidently predicting properties for a different molecule is worse than
# refusing. See crates/admet-infer/src/lib.rs::MAX_ATOMS and ADR-03.
MAX_HEAVY_ATOMS = 128


def profile_frame(df: pd.DataFrame, task: str) -> dict[str, object]:
    """Profile one endpoint's rows. Mirrors manual Listing 18.1's profile()."""
    mols = [Chem.MolFromSmiles(s) for s in df[SMILES_COL]]
    valid = [m for m in mols if m is not None]
    heavy = [m.GetNumHeavyAtoms() for m in valid]
    y = pd.to_numeric(df[LABEL_COL], errors="coerce")

    out: dict[str, object] = {
        "rows": len(df),
        "unparseable": len(mols) - len(valid),
        "heavy_mean": round(sum(heavy) / len(heavy), 2) if heavy else 0.0,
        "heavy_max": max(heavy) if heavy else 0,
        "over_cap": sum(1 for h in heavy if h > MAX_HEAVY_ATOMS),
        "duplicate_smiles": int(df[SMILES_COL].duplicated().sum()),
    }
    out["over_cap_pct"] = round(100.0 * int(out["over_cap"]) / max(len(valid), 1), 3)

    if task == "binary":
        # Positive rate, not accuracy-friendly balance. Several endpoints run
        # below 20% positives, which is why the report uses AUROC and PR-AUC and
        # never accuracy: at 15% positives, predicting "negative" scores 85%.
        pos = float(y.mean())
        out["positive_rate"] = round(pos, 4)
        out["imbalanced"] = pos < 0.2 or pos > 0.8
        # pos_weight for BCEWithLogitsLoss, computed here so the trainer does
        # not recompute it from a different split and get a different number.
        out["pos_weight"] = round((1 - pos) / pos, 3) if 0 < pos < 1 else None
    else:
        out["target_mean"] = round(float(y.mean()), 4)
        out["target_std"] = round(float(y.std()), 4)
        out["target_min"] = round(float(y.min()), 4)
        out["target_max"] = round(float(y.max()), 4)
        # Skew flags the long-tailed endpoints. Half-life and clearance have
        # genuine outliers, which is why the loss is Huber rather than MSE:
        # MSE lets one extreme compound dominate the gradient.
        out["target_skew"] = round(float(y.skew()), 3)
        out["long_tailed"] = abs(float(y.skew())) > 1.0

    return out


def main() -> int:
    manifest_path = RAW_DIR / "_manifest.json"
    if not manifest_path.exists():
        print(
            f"No download manifest at {manifest_path.as_posix()}.\n"
            "Run the download first (in .venv-tdc):\n"
            "    just data-download",
            file=sys.stderr,
        )
        return 1

    profiles: dict[str, object] = {}
    warnings: list[str] = []

    header = (
        f"{'key':<10} {'task':<10} {'rows':>6} {'bad':>4} "
        f"{'heavy~':>7} {'max':>4} {'>128':>5} {'dup':>4}  distribution"
    )
    print(header)
    print("-" * len(header))

    for key, (tdc_name, category, task, expected_n) in ENDPOINTS.items():
        frames = []
        for split in ("train_val", "test"):
            path = RAW_DIR / f"{key}_{split}.csv"
            if not path.exists():
                warnings.append(f"{key}: missing {split} split")
                continue
            frames.append(pd.read_csv(path))

        if not frames:
            print(f"{key:<10} {task:<10} {'--':>6}  MISSING")
            continue

        df = pd.concat(frames, ignore_index=True)
        p = profile_frame(df, task)
        p |= {"tdc_name": tdc_name, "category": category, "task": task}
        profiles[key] = p

        if task == "binary":
            dist = f"positives {p['positive_rate']:.1%}"
            if p["imbalanced"]:
                dist += "  IMBALANCED"
                warnings.append(
                    f"{key}: {p['positive_rate']:.1%} positives -- use pos_weight={p['pos_weight']}"
                )
        else:
            dist = f"mean {p['target_mean']:.2f} sd {p['target_std']:.2f} skew {p['target_skew']:+.2f}"
            if p["long_tailed"]:
                dist += "  LONG-TAILED"
                warnings.append(
                    f"{key}: skew {p['target_skew']:+.2f} -- Huber loss, not MSE"
                )

        print(
            f"{key:<10} {task:<10} {p['rows']:>6} {p['unparseable']:>4} "
            f"{p['heavy_mean']:>7.1f} {p['heavy_max']:>4} {p['over_cap']:>5} "
            f"{p['duplicate_smiles']:>4}  {dist}"
        )

        if p["unparseable"]:
            warnings.append(f"{key}: {p['unparseable']} unparseable SMILES dropped")
        if p["over_cap"]:
            warnings.append(
                f"{key}: {p['over_cap']} molecules ({p['over_cap_pct']}%) exceed the "
                f"{MAX_HEAVY_ATOMS}-atom cap and will be REJECTED"
            )

    total_rows = sum(int(p["rows"]) for p in profiles.values())  # type: ignore[index]
    print("-" * len(header))
    print(f"{'TOTAL':<10} {'':<10} {total_rows:>6}")

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    out_path = RESULTS_DIR / "data_profile.json"
    out_path.write_text(
        json.dumps(
            {
                "max_heavy_atoms": MAX_HEAVY_ATOMS,
                "total_rows": total_rows,
                "endpoints": profiles,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"\nprofile -> {out_path.relative_to(REPO_ROOT).as_posix()}")

    if warnings:
        print(f"\n{len(warnings)} thing(s) worth knowing before you model:")
        for w in warnings:
            print(f"  - {w}")
        print(
            "\nNone of these are errors. They are the findings that make the data\n"
            "chapter of your report worth reading -- record them in your weekly log."
        )

    # The multi-task argument, quantified. This is the number that justifies one
    # shared trunk over twelve separate models: hERG's 648 rows cannot learn a
    # molecular representation, but they can fit a head on top of one learned
    # from every row across every endpoint.
    print(f"\nShared trunk sees ~{total_rows:,} labelled rows across 12 endpoints.")
    print(
        "The smallest endpoint alone has far fewer -- that gap is why the model is multi-task."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
