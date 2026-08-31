"""One-off check: does rows - duplicate_smiles equal the documented row count?

The downloader warns DRIFT for five endpoints. The hypothesis is that the counts
in docs/06-data-sources.md are UNIQUE-molecule counts while the TDC benchmark-group
CSVs contain duplicate SMILES, in which case there is no drift at all and nobody
should go looking for a data-versioning problem.
"""

import json
from pathlib import Path

DOCUMENTED = {
    "caco2": 906,
    "hia": 578,
    "pgp": 1212,
    "bioavail": 640,
    "bbb": 1975,
    "ppbr": 1797,
    "vdss": 1130,
    "cyp3a4": 12328,
    "cyp2d6": 13130,
    "half_life": 667,
    "clearance": 1213,
    "herg": 648,
}

profile = json.loads(Path("results/data_profile.json").read_text())
endpoints = profile.get("endpoints", profile)

print(f"{'key':11}{'rows':>7}{'dup':>6}{'rows-dup':>10}{'documented':>12}  verdict")
print("-" * 60)

explained = unexplained = 0
for key, doc in DOCUMENTED.items():
    entry = endpoints.get(key)
    if not isinstance(entry, dict):
        print(f"{key:11}{'?':>7}  (not in profile)")
        continue

    rows = entry.get("rows")
    dup = next(
        (entry[k] for k in ("dup", "duplicates", "duplicate_smiles") if k in entry),
        None,
    )
    if rows is None or dup is None:
        print(f"{key:11}{rows!s:>7}  (missing keys: {sorted(entry)})")
        continue

    unique = rows - dup
    if unique == doc:
        verdict = "explained by duplicates"
        explained += 1
    elif rows == doc:
        verdict = "no drift"
        explained += 1
    else:
        verdict = f"UNEXPLAINED (off by {unique - doc:+d})"
        unexplained += 1
    print(f"{key:11}{rows:>7}{dup:>6}{unique:>10}{doc:>12}  {verdict}")

print("-" * 60)
print(f"explained: {explained}   unexplained: {unexplained}")
