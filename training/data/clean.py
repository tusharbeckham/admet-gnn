"""
Generic molecule cleaning: parse, strip salts, canonicalise, deduplicate.

Dataset-agnostic on purpose. Nothing in this module knows about TDC, about
MoleculeNet, or about any particular column name -- callers pass those in. That
is what let the MoleculeNet prototype and the TDC pipeline share it unchanged
when the endpoints moved (see MIGRATION.md), and it is why the endpoint-specific
wrappers live elsewhere:

    training/data/clean_tdc.py                  <- current, Increment 1
    training/legacy_moleculenet/clean_moleculenet.py   <- superseded prototype

Manual references: ch. 8 (canonicalisation and identity), method.md §2 step 2.
"""

from __future__ import annotations

from pathlib import Path

import pandas as pd
from rdkit import Chem
from rdkit.Chem.SaltRemover import SaltRemover

REPO_ROOT = Path(__file__).resolve().parents[2]
RAW_DIR = Path(__file__).parent / "raw"
PROCESSED_DIR = Path(__file__).parent / "processed"

# The model's hard cap, mirrored from crates/admet-infer/src/lib.rs::MAX_ATOMS.
# Molecules above it are REJECTED, never truncated. A truncated molecule is a
# different molecule, and returning a confident prediction about a different
# molecule is worse than returning nothing. See ADR-03.
MAX_HEAVY_ATOMS = 128

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


def inchikey(smiles: str) -> str | None:
    """
    The 27-character cross-database identity key.

    This is the same key the Rust cache uses, so a molecule seen in training and
    the same molecule seen at inference resolve identically. Fixed width makes it
    an ideal CHAR(27) primary key with a compact B-tree index -- see ADR-04 and
    manual Table 8.2.

    Aspirin is BSYNRYMUTXBXSQ-UHFFFAOYSA-N: 14 chars for the skeleton, 10 for
    stereochemistry and isotopes, 1 for protonation.
    """
    if not isinstance(smiles, str) or not smiles.strip():
        return None
    mol = Chem.MolFromSmiles(smiles)
    return None if mol is None else Chem.MolToInchiKey(mol)


def clean_dataset(
    df: pd.DataFrame,
    smiles_col: str,
    label_cols: list[str],
    task_type: str,
    *,
    add_inchikey: bool = True,
    reject_oversized: bool = True,
) -> pd.DataFrame:
    """
    Parse/canonicalise SMILES, drop invalid rows, reject oversized molecules,
    and collapse duplicates: mean for regression, majority vote for
    classification.

    Every drop count is printed, so dataset shrinkage is never silent. A
    dataset that loses 5% of its rows here is a finding worth a paragraph in the
    report, not a number to discover in week nine.

    Deduplication is on InChIKey when available, because that is the only key
    that treats `CCO`, `OCC` and `C(C)O` as the one molecule they are. Falling
    back to canonical SMILES is nearly as good and much better than raw input.
    """
    df = df.copy()
    df["canonical_smiles"] = df[smiles_col].apply(canonicalize)

    n_before = len(df)
    df = df.dropna(subset=["canonical_smiles"])
    n_parsed = len(df)
    print(
        f"  parsed {n_parsed}/{n_before} rows ({n_before - n_parsed} dropped as invalid SMILES)"
    )

    if reject_oversized:
        heavy = df["canonical_smiles"].apply(
            lambda s: Chem.MolFromSmiles(s).GetNumHeavyAtoms()
        )
        df["heavy_atoms"] = heavy
        oversized = int((heavy > MAX_HEAVY_ATOMS).sum())
        if oversized:
            df = df[heavy <= MAX_HEAVY_ATOMS]
            print(
                f"  rejected {oversized} molecules over {MAX_HEAVY_ATOMS} heavy atoms "
                f"({100.0 * oversized / n_parsed:.2f}%) -- not truncated, rejected"
            )

    key_col = "canonical_smiles"
    if add_inchikey:
        df["inchikey"] = df["canonical_smiles"].apply(inchikey)
        n_keyed = int(df["inchikey"].notna().sum())
        if n_keyed == len(df):
            key_col = "inchikey"
        else:
            print(
                f"  WARNING: InChIKey failed for {len(df) - n_keyed} rows; "
                f"deduplicating on canonical SMILES instead"
            )

    # Keep one representative canonical SMILES per identity group alongside the
    # aggregated labels -- the trainer needs a structure to featurise, and the
    # groupby key alone is not one when the key is an InChIKey.
    agg: dict[str, object] = {"canonical_smiles": "first"}
    if "heavy_atoms" in df.columns:
        agg["heavy_atoms"] = "first"

    n_pre_dedup = len(df)
    if task_type == "regression":
        agg |= {c: "mean" for c in label_cols}
    else:

        def majority_vote(s: pd.Series):
            modes = s.mode()
            return modes.iloc[0] if not modes.empty else None

        agg |= {c: majority_vote for c in label_cols}

    grouped = df.groupby(key_col, as_index=False).agg(agg)

    n_collapsed = n_pre_dedup - len(grouped)
    print(
        f"  collapsed {n_pre_dedup} rows into {len(grouped)} unique molecules", end=""
    )
    print(f" ({n_collapsed} duplicates merged)" if n_collapsed else "")

    return grouped


def label_conflict_report(
    df: pd.DataFrame, key_col: str, label_col: str, task_type: str
) -> pd.DataFrame:
    """
    Rows where the same molecule carries disagreeing labels.

    Worth running once per endpoint and recording the count. A high conflict rate
    is a data-quality signal about the underlying assay, not a bug in this code,
    and it belongs in the report -- it is the kind of honest observation that
    reads as domain understanding rather than box-ticking.
    """
    grouped = df.groupby(key_col)[label_col]
    if task_type == "regression":
        # Any non-zero spread among duplicates is a disagreement.
        conflicted = grouped.std().pipe(lambda s: s[s > 1e-9])
    else:
        # More than one distinct label for the same molecule.
        conflicted = grouped.nunique().pipe(lambda s: s[s > 1])
    conflicted = conflicted[conflicted.index.notna()]
    return (
        df[df[key_col].isin(conflicted.index)]
        .sort_values(key_col)
        .loc[:, [key_col, label_col]]
    )
