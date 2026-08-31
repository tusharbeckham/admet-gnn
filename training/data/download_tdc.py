"""
Download the twelve TDC ADMET benchmark endpoints as raw CSV.

    # in the SEPARATE tdc environment -- see docs/00-machine-setup.md
    .venv-tdc/Scripts/python.exe training/data/download_tdc.py
    # or:
    just data-download

Produces, for each endpoint:
    training/data/raw/tdc/<key>_train_val.csv
    training/data/raw/tdc/<key>_test.csv
    training/data/raw/tdc/_manifest.json

===============================================================================
 HARD RULE: THIS SCRIPT PERFORMS NO CHEMISTRY
===============================================================================
It fetches and writes raw SMILES and labels. Nothing else. No RDKit import, no
canonicalisation, no salt stripping, no scaffold assignment, no featurisation.

Why that matters enough to be shouted about: PyTDC 1.1.15 pins
`rdkit>=2023.9.5,<2024.3.1`, while the project's real environment runs rdkit
2026.3.5. So this script runs in its own throwaway `.venv-tdc`. If chemistry
happened here it would happen against a two-and-a-half-year-old RDKit, and the
resulting features would silently disagree with the ones the trainer and the
Rust server compute.

That is risk R3 (feature skew), and it is invisible until metrics disagree and
you have no idea which of four layers is lying. Keeping the seam at "raw bytes
in, raw bytes out" makes the class of bug impossible rather than unlikely.

Every RDKit operation belongs in `.venv`, downstream of this file.
===============================================================================

Manual references: ch. 3.2 (Listing 3.2), ch. 3.3 (Table 3.1), ch. 18.2 step 1.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

# NOTE the absence of `from rdkit import Chem`. See the banner above. If you
# ever feel the urge to add it, add a new script in .venv instead.

REPO_ROOT = Path(__file__).resolve().parents[2]
RAW_DIR = REPO_ROOT / "training" / "data" / "raw" / "tdc"

# The twelve shipped endpoints. TDC offers twenty-two; twelve is the right scope
# for fifteen weeks -- broad enough to be a real platform, small enough to
# finish -- and these twelve span all five ADMET categories, which is what lets
# the report claim genuine coverage.
#
# The endpoint list is FROZEN. Adding a thirteenth requires a written ADR
# (risk R5, scope creep). Sizes are TDC's published counts, recorded here so a
# download that returns a different number is caught immediately rather than
# quietly changing every metric downstream.
#
#   key            TDC dataset name           category      task         n
ENDPOINTS: dict[str, tuple[str, str, str, int]] = {
    "caco2": ("Caco2_Wang", "absorption", "regression", 906),
    "hia": ("HIA_Hou", "absorption", "binary", 578),
    "pgp": ("Pgp_Broccatelli", "absorption", "binary", 1_212),
    "bioavail": ("Bioavailability_Ma", "absorption", "binary", 640),
    "bbb": ("BBB_Martins", "distribution", "binary", 1_975),
    "ppbr": ("PPBR_AZ", "distribution", "regression", 1_797),
    "vdss": ("VDss_Lombardo", "distribution", "regression", 1_130),
    "cyp3a4": ("CYP3A4_Veith", "metabolism", "binary", 12_328),
    "cyp2d6": ("CYP2D6_Veith", "metabolism", "binary", 13_130),
    "half_life": ("Half_Life_Obach", "excretion", "regression", 667),
    "clearance": ("Clearance_Hepatocyte_AZ", "excretion", "regression", 1_213),
    "herg": ("hERG", "toxicity", "binary", 648),
}

# TDC prescribes five split seeds for the train/validation split. Report mean
# and standard deviation across all five -- a single run is not a result, and
# examiners notice. Recorded here so the trainer reads it from one place.
PRESCRIBED_SEEDS = [1, 2, 3, 4, 5]

# TDC's own column names. 'Drug' holds the SMILES string, which is not obvious
# from the name and has caught people out.
SMILES_COL = "Drug"
LABEL_COL = "Y"
ID_COL = "Drug_ID"


def download_all(force: bool = False) -> int:
    """Fetch every endpoint. Returns a process exit code."""
    try:
        from tdc.benchmark_group import admet_group
    except ImportError:
        print(
            "PyTDC is not importable.\n\n"
            "This script must run in the SEPARATE tdc environment, not .venv:\n"
            "    uv venv .venv-tdc --python 3.12\n"
            "    .venv-tdc/Scripts/python.exe -m pip install -r requirements-data.txt\n"
            "    .venv-tdc/Scripts/python.exe training/data/download_tdc.py\n\n"
            "See docs/00-machine-setup.md and requirements-data.txt for why the\n"
            "environments are split.",
            file=sys.stderr,
        )
        return 1

    RAW_DIR.mkdir(parents=True, exist_ok=True)

    #  PyTDC 1.1.15 exposes no `tdc.__version__`, so reading it crashed the
    #  downloader before it fetched a single byte. The installed distribution
    #  metadata is authoritative anyway -- a package that forgets to set
    #  `__version__` still has a version.
    try:
        from importlib.metadata import version as _dist_version

        tdc_version = _dist_version("PyTDC")
    except Exception:  # noqa: BLE001
        #  Never fail the download over a version string. Knowing which TDC
        #  produced a dataset is useful; it is not worth losing the dataset for.
        tdc_version = "unknown"

    # TDC downloads into `path` on first call and caches afterwards. Point it at
    # the raw dir so the cache is gitignored along with everything else large.
    print(f"PyTDC {tdc_version}")
    print(f"cache/download dir: {RAW_DIR.as_posix()}\n")
    group = admet_group(path=str(RAW_DIR))

    manifest: dict[str, object] = {
        "pytdc_version": tdc_version,
        "smiles_column": SMILES_COL,
        "label_column": LABEL_COL,
        "prescribed_seeds": PRESCRIBED_SEEDS,
        "endpoints": {},
    }
    failures: list[str] = []

    for key, (tdc_name, category, task, expected_n) in ENDPOINTS.items():
        train_val_path = RAW_DIR / f"{key}_train_val.csv"
        test_path = RAW_DIR / f"{key}_test.csv"

        if train_val_path.exists() and test_path.exists() and not force:
            import csv

            with train_val_path.open(newline="", encoding="utf-8") as fh:
                n_tv = sum(1 for _ in csv.reader(fh)) - 1
            with test_path.open(newline="", encoding="utf-8") as fh:
                n_te = sum(1 for _ in csv.reader(fh)) - 1
            print(f"[skip]     {key:<10} {tdc_name:<26} present ({n_tv} + {n_te})")
            manifest["endpoints"][key] = {  # type: ignore[index]
                "tdc_name": tdc_name,
                "category": category,
                "task": task,
                "n_train_val": n_tv,
                "n_test": n_te,
                "n_total": n_tv + n_te,
                "n_expected": expected_n,
            }
            continue

        try:
            benchmark = group.get(tdc_name)
        except Exception as exc:  # noqa: BLE001 -- report and continue
            print(f"[FAIL]     {key:<10} {tdc_name:<26} {type(exc).__name__}: {exc}")
            failures.append(key)
            continue

        train_val = benchmark["train_val"]
        test = benchmark["test"]

        train_val.to_csv(train_val_path, index=False)
        test.to_csv(test_path, index=False)

        n_total = len(train_val) + len(test)

        #  Compare on UNIQUE SMILES, not raw rows.
        #
        #  The published TDC figures in `docs/06-data-sources.md` are
        #  unique-molecule counts, while the benchmark-group CSVs contain repeated
        #  SMILES — 993 of `ppbr`'s 2,790 rows, and 55 of `bbb`'s 2,030. Comparing
        #  raw rows therefore reported DRIFT on five endpoints that had not
        #  drifted at all, which is an afternoon lost hunting a data-versioning
        #  problem that does not exist. Verified for all twelve endpoints:
        #  rows - duplicates reproduces the published figure exactly, or the raw
        #  count already matches.
        #
        #  Genuine drift is still worth shouting about — a benchmark number is
        #  meaningless without knowing which dataset revision produced it — so the
        #  check is kept, just made correct.
        smiles_col = next(
            (c for c in ("Drug", "smiles", "SMILES", "drug") if c in train_val.columns),
            None,
        )
        if smiles_col is None:
            n_unique = n_total
            dup_note = "  (no SMILES column found; comparing raw rows)"
        else:
            import pandas as _pd

            all_smiles = _pd.concat([train_val[smiles_col], test[smiles_col]])
            n_unique = int(all_smiles.nunique())
            n_dup = n_total - n_unique
            dup_note = f"  ({n_dup} duplicate SMILES)" if n_dup else ""

        drift = (
            ""
            if n_unique == expected_n or n_total == expected_n
            else f"  <-- expected {expected_n} unique, got {n_unique}, DRIFT"
        )
        print(
            f"[download] {key:<10} {tdc_name:<26} "
            f"{len(train_val):>6} + {len(test):>5} = {n_total:>6}"
            f"{dup_note}{drift}"
        )

        manifest["endpoints"][key] = {  # type: ignore[index]
            "tdc_name": tdc_name,
            "category": category,
            "task": task,
            "n_train_val": len(train_val),
            "n_test": len(test),
            "n_total": n_total,
            #  Recorded because it, not `n_total`, is the number comparable with
            #  TDC's published figures and with any other paper's row count.
            "n_unique_smiles": n_unique,
            "n_expected": expected_n,
            "columns": list(train_val.columns),
        }

    (RAW_DIR / "_manifest.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )

    print(
        f"\nmanifest -> {(RAW_DIR / '_manifest.json').relative_to(REPO_ROOT).as_posix()}"
    )

    if failures:
        print(
            f"\n{len(failures)} endpoint(s) failed: {', '.join(failures)}",
            file=sys.stderr,
        )
        print("Re-run to retry -- successful endpoints are skipped.", file=sys.stderr)
        return 1

    print(f"\nAll {len(ENDPOINTS)} endpoints present.")
    print("\nNext, in the MAIN environment (.venv):")
    print("    just data-profile      # exploratory summary before any modelling")
    return 0


if __name__ == "__main__":
    raise SystemExit(download_all(force="--force" in sys.argv))
