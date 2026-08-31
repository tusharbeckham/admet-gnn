"""
Bemis-Murcko scaffold splitting for molecular datasets.

Splits by whole scaffold GROUP, not by individual molecule, so no scaffold
appears in more than one split. This is what makes the split a genuine test
of generalization to new chemistry, per MoleculeNet's own recommendation.
Never swap this for a random split on these datasets -- see research.md §6.
"""

from __future__ import annotations

import random
from collections import defaultdict

import pandas as pd
from rdkit import Chem
from rdkit.Chem.Scaffolds import MurckoScaffold


def get_scaffold(smiles: str, include_chirality: bool = False) -> str:
    """Return the Bemis-Murcko scaffold SMILES for a molecule (empty string on parse failure)."""
    mol = Chem.MolFromSmiles(smiles)
    if mol is None:
        return ""
    scaffold = MurckoScaffold.GetScaffoldForMol(mol)
    return Chem.MolToSmiles(scaffold, isomericSmiles=include_chirality)


def scaffold_split(
    df: pd.DataFrame,
    smiles_col: str = "canonical_smiles",
    frac_train: float = 0.8,
    frac_val: float = 0.1,
    frac_test: float = 0.1,
    seed: int = 42,
) -> tuple[pd.DataFrame, pd.DataFrame, pd.DataFrame]:
    """Split a dataframe into train/val/test by scaffold group."""
    assert abs(frac_train + frac_val + frac_test - 1.0) < 1e-6, (
        "fractions must sum to 1.0"
    )

    scaffolds: dict[str, list[int]] = defaultdict(list)
    for idx, smiles in enumerate(df[smiles_col]):
        scaffolds[get_scaffold(smiles)].append(idx)

    # Sort scaffold groups biggest-first, then greedily fill train, then val,
    # then whatever's left goes to test. Shuffle first so ties among
    # same-size groups aren't determined by input row order.
    rng = random.Random(seed)
    groups = list(scaffolds.values())
    rng.shuffle(groups)
    groups.sort(key=len, reverse=True)

    n_total = len(df)
    n_train_target = int(frac_train * n_total)
    n_val_target = int(frac_val * n_total)

    train_idx: list[int] = []
    val_idx: list[int] = []
    test_idx: list[int] = []

    for group in groups:
        if len(train_idx) + len(group) <= n_train_target:
            train_idx.extend(group)
        elif len(val_idx) + len(group) <= n_val_target:
            val_idx.extend(group)
        else:
            test_idx.extend(group)

    train_df = df.iloc[train_idx].reset_index(drop=True)
    val_df = df.iloc[val_idx].reset_index(drop=True)
    test_df = df.iloc[test_idx].reset_index(drop=True)

    print(
        f"  scaffold split: train={len(train_df)} ({len(train_df) / n_total:.1%}), "
        f"val={len(val_df)} ({len(val_df) / n_total:.1%}), "
        f"test={len(test_df)} ({len(test_df) / n_total:.1%})"
    )
    return train_df, val_df, test_df


if __name__ == "__main__":
    # Self-test: benzene/toluene share a scaffold and must land in the same
    # split; naphthalene and ethanol have different scaffolds.
    test_df = pd.DataFrame(
        {
            "canonical_smiles": [
                "c1ccccc1",  # benzene
                "Cc1ccccc1",  # toluene (same benzene scaffold)
                "c1ccc2ccccc2c1",  # naphthalene (different scaffold)
                "CCO",  # ethanol (no ring -> empty scaffold)
            ]
        }
    )
    train, val, test = scaffold_split(
        test_df, frac_train=0.5, frac_val=0.25, frac_test=0.25, seed=0
    )

    benzene_split = (
        "train"
        if 0 in train.index or "c1ccccc1" in train["canonical_smiles"].values
        else None
    )
    assert ("c1ccccc1" in train["canonical_smiles"].values) == (
        "Cc1ccccc1" in train["canonical_smiles"].values
    ), "benzene and toluene share a scaffold and must land in the same split"
    print("Self-test passed: shared-scaffold molecules stayed together.")
