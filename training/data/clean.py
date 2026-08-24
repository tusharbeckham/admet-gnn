"""
Clean raw MoleculeNet CSVs: parse SMILES with RDKit, strip salts, canonicalize,
drop unparseable rows, and collapse duplicate molecules.

This module is generic — dataset-specific column names live in clean_esol(),
clean_bbbp(), clean_tox21() below. If a raw file's columns don't match what's
here, open it once with pandas and check df.columns before editing.
"""
from __future__ import annotations

from pathlib import Path

import pandas as pd
from rdkit import Chem
from rdkit.Chem.SaltRemover import SaltRemover

RAW_DIR = Path(__file__).parent / "raw"
PROCESSED_DIR = Path(__file__).parent / "processed"

_remover = SaltRemover()


def canonicalize(smiles: str) -> str | None:
    """Parse a SMILES string, strip salts, return canonical SMILES or None."""
    if not isinstance(smiles, str) or not smiles.strip():
        return None
    mol = Chem.MolFromSmiles(smiles)
    if mol is None:
        return None
    mol = _remover.StripMol(mol, dontRemoveEverything=True)
    if mol is None or mol.GetNumAtoms() == 0:
        return None
    return Chem.MolToSmiles(mol, canonical=True)


def clean_dataset(
    df: pd.DataFrame,
    smiles_col: str,
    label_cols: list[str],
    task_type: str,
) -> pd.DataFrame:
    """
    Parse/canonicalize SMILES, drop invalid rows, and collapse duplicate
    molecules: average labels for regression, majority-vote for
    classification. Rows are printed with drop counts so shrinkage is
    never silent.
    """
    df = df.copy()
    df["canonical_smiles"] = df[smiles_col].apply(canonicalize)

    n_before = len(df)
    df = df.dropna(subset=["canonical_smiles"])
    n_parsed = len(df)
    print(f"  parsed {n_parsed}/{n_before} rows ({n_before - n_parsed} dropped as invalid SMILES)")

    if task_type == "regression":
        grouped = df.groupby("canonical_smiles")[label_cols].mean().reset_index()
    else:
        def majority_vote(s: pd.Series):
            modes = s.mode()
            return modes.iloc[0] if not modes.empty else None
        grouped = df.groupby("canonical_smiles")[label_cols].agg(majority_vote).reset_index()

    print(f"  collapsed {n_parsed} rows into {len(grouped)} unique molecules")
    return grouped


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
    # Verify this against df.columns if the raw file schema ever changes.
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
        "NR-AR", "NR-AR-LBD", "NR-AhR", "NR-Aromatase", "NR-ER", "NR-ER-LBD",
        "NR-PPAR-gamma", "SR-ARE", "SR-ATAD5", "SR-HSE", "SR-MMP", "SR-p53",
    ]
    missing = [c for c in task_cols if c not in raw.columns]
    if missing:
        raise ValueError(
            f"Expected Tox21 columns not found: {missing}. "
            f"Actual columns: {list(raw.columns)}"
        )
    # NOTE: majority-vote on 12 sparsely-populated columns will turn any
    # NaN-only group into NaN, which is correct (unmeasured stays unmeasured)
    # -- do not fillna before this step.
    return clean_dataset(raw, smiles_col="smiles", label_cols=task_cols, task_type="classification")


if __name__ == "__main__":
    PROCESSED_DIR.mkdir(parents=True, exist_ok=True)

    esol = clean_esol()
    esol.to_csv(PROCESSED_DIR / "esol_cleaned.csv", index=False)

    bbbp = clean_bbbp()
    bbbp.to_csv(PROCESSED_DIR / "bbbp_cleaned.csv", index=False)

    tox21 = clean_tox21()
    tox21.to_csv(PROCESSED_DIR / "tox21_cleaned.csv", index=False)

    print("\nSaved cleaned CSVs to data/processed/")