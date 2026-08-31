"""
MoleculeNet-specific cleaning wrappers. SUPERSEDED -- see ./README.md.

Kept because these three functions are the concrete evidence behind the
MoleculeNet -> TDC migration recorded in MIGRATION.md and ADR-06. Do not import
them from anything under `training/data/`.

The generic machinery these call (`canonicalize`, `clean_dataset`) still lives
at `training/data/clean.py` and is used unchanged by the TDC pipeline. That it
survived the migration untouched is the point: the dataset-specific knowledge
was isolated in wrappers like these, so swapping the dataset cost three
functions rather than a rewrite.
"""

from __future__ import annotations

import sys
from pathlib import Path

import pandas as pd

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "data"))

from clean import clean_dataset  # import deliberately after the path shim above

# The MoleculeNet prototype downloaded into the repo-root data/raw/, not
# training/data/raw/. Those CSVs are still on disk and still gitignored.
RAW_DIR = Path(__file__).resolve().parents[2] / "data" / "raw"


def clean_esol() -> pd.DataFrame:
    print("Cleaning ESOL...")
    raw = pd.read_csv(RAW_DIR / "delaney-processed.csv")
    return clean_dataset(
        raw,
        smiles_col="smiles",
        label_cols=["measured log solubility in mols per litre"],
        task_type="regression",
    )


def clean_bbbp() -> pd.DataFrame:
    print("Cleaning BBBP...")
    raw = pd.read_csv(RAW_DIR / "BBBP.csv")
    # BBBP's label column is `p_np` (1 = penetrant, 0 = non-penetrant).
    return clean_dataset(
        raw,
        smiles_col="smiles",
        label_cols=["p_np"],
        task_type="classification",
    )


def clean_tox21() -> pd.DataFrame:
    print("Cleaning Tox21...")
    raw = pd.read_csv(RAW_DIR / "tox21.csv")
    task_cols = [
        "NR-AR",
        "NR-AR-LBD",
        "NR-AhR",
        "NR-Aromatase",
        "NR-ER",
        "NR-ER-LBD",
        "NR-PPAR-gamma",
        "SR-ARE",
        "SR-ATAD5",
        "SR-HSE",
        "SR-MMP",
        "SR-p53",
    ]
    missing = [c for c in task_cols if c not in raw.columns]
    if missing:
        raise ValueError(
            f"Expected Tox21 columns not found: {missing}. "
            f"Actual columns: {list(raw.columns)}"
        )
    # NOTE: majority-vote on 12 sparsely-populated columns turns any NaN-only
    # group into NaN, which is correct (unmeasured stays unmeasured) -- do not
    # fillna before this step.
    return clean_dataset(
        raw, smiles_col="smiles", label_cols=task_cols, task_type="classification"
    )
