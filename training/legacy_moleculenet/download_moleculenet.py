"""
Download raw MoleculeNet CSVs into data/raw/.

Run: python data/download.py
"""

from __future__ import annotations

import gzip
import shutil
from pathlib import Path
from urllib.request import urlretrieve

RAW_DIR = Path(__file__).parent / "raw"

# Confirmed-live URLs (checked 2026-07-22) on the deepchemdata S3 bucket that
# MoleculeNet's own tutorials and papers link to directly.
DATASETS = {
    "esol": "https://deepchemdata.s3-us-west-1.amazonaws.com/datasets/delaney-processed.csv",
    "bbbp": "https://deepchemdata.s3-us-west-1.amazonaws.com/datasets/BBBP.csv",
    "tox21": "https://deepchemdata.s3-us-west-1.amazonaws.com/datasets/tox21.csv.gz",
}


def download_all() -> None:
    RAW_DIR.mkdir(parents=True, exist_ok=True)
    for name, url in DATASETS.items():
        dest = RAW_DIR / Path(url).name
        if dest.exists():
            print(f"[skip] {name}: already present at {dest}")
            continue

        print(f"[download] {name} -> {dest}")
        urlretrieve(url, dest)

        if dest.suffix == ".gz":
            unzipped = dest.with_suffix("")
            with gzip.open(dest, "rb") as f_in, open(unzipped, "wb") as f_out:
                shutil.copyfileobj(f_in, f_out)
            print(f"[unzip] {dest.name} -> {unzipped.name}")


if __name__ == "__main__":
    download_all()
    print("\nDone. Contents of data/raw/:")
    for f in sorted(RAW_DIR.glob("*")):
        print(" ", f.name)
