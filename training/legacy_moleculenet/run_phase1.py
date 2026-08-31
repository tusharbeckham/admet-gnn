"""
Phase 1 end-to-end: download -> clean -> scaffold-split -> sanity-check -> save.

Run: python data/run_phase1.py
Produces: data/processed/<dataset>/{train,val,test}.csv
"""

from __future__ import annotations

from pathlib import Path

import pandas as pd
from clean import clean_bbbp, clean_esol, clean_tox21
from download import download_all
from scaffold_split import scaffold_split

PROCESSED_DIR = Path(__file__).parent / "processed"

TOX21_TASKS = [
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


def sanity_check_regression(
    name: str, splits: dict[str, pd.DataFrame], label_col: str
) -> None:
    print(f"\n[{name}] target distribution per split:")
    for split_name, split_df in splits.items():
        vals = split_df[label_col]
        print(
            f"  {split_name}: n={len(vals)}, mean={vals.mean():.3f}, std={vals.std():.3f}"
        )


def sanity_check_classification(
    name: str, splits: dict[str, pd.DataFrame], label_cols: list[str]
) -> None:
    print(f"\n[{name}] positive-class rate per split (NaN = unmeasured, excluded):")
    for split_name, split_df in splits.items():
        rates = split_df[label_cols].mean(skipna=True)
        overall = rates.mean()
        print(
            f"  {split_name}: n={len(split_df)}, mean positive rate across tasks={overall:.3f}"
        )
        for col in label_cols:
            if split_df[col].notna().sum() == 0:
                print(f"    WARNING: {col} has zero labeled examples in {split_name}")


def process_dataset(
    name: str,
    df: pd.DataFrame,
    label_cols: list[str],
    task_type: str,
) -> None:
    out_dir = PROCESSED_DIR / name
    out_dir.mkdir(parents=True, exist_ok=True)

    train, val, test = scaffold_split(df, seed=42)
    splits = {"train": train, "val": val, "test": test}

    for split_name, split_df in splits.items():
        split_df.to_csv(out_dir / f"{split_name}.csv", index=False)

    if task_type == "regression":
        sanity_check_regression(name, splits, label_cols[0])
    else:
        sanity_check_classification(name, splits, label_cols)


def main() -> None:
    print("=== Step 1: download raw CSVs ===")
    download_all()

    print("\n=== Step 2/3: clean + scaffold-split each dataset ===")

    esol = clean_esol()
    process_dataset(
        "esol", esol, ["measured log solubility in mols per litre"], "regression"
    )

    bbbp = clean_bbbp()
    process_dataset("bbbp", bbbp, ["p_np"], "classification")

    tox21 = clean_tox21()
    process_dataset("tox21", tox21, TOX21_TASKS, "classification")

    print("\nPhase 1 complete. Check data/processed/<dataset>/{train,val,test}.csv")


if __name__ == "__main__":
    main()
