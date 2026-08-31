"""The Python side of the 33-feature contract.

`models/feature_schema.json` is generated from Rust (`just schema`) and read by the
Python featuriser. `crates/admet-cli/tests/feature_schema_contract.rs` guards the
producing end — that the committed file still matches the code that made it.

These tests guard the *consuming* end: that the file contains what the Python
featuriser is about to assume about it. Both halves are needed. A contract checked
only by its author is a comment.

The failure this prevents has no symptom. If Python lays out the 33 columns
differently from Rust, nothing raises: the model trains on one layout and is served
another, and the only evidence is an accuracy regression that looks like a modelling
problem. That is the specific hazard ADR-01 exists to remove, and it is why the
boundary is one exported artefact rather than two implementations kept in step by
good intentions.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = REPO_ROOT / "models" / "feature_schema.json"


@pytest.fixture(scope="module")
def schema() -> dict:
    if not SCHEMA_PATH.exists():
        pytest.fail(
            f"{SCHEMA_PATH} is missing. It is the Python<->Rust feature contract "
            "and is committed on purpose. Regenerate it with: just schema"
        )
    return json.loads(SCHEMA_PATH.read_text())


def test_contract_is_33_dimensional(schema: dict) -> None:
    # The number the exported ONNX graph is built around. Changing it invalidates
    # every fixture and every trained model at once.
    assert schema["n_features"] == 33


def test_schema_version_is_present_and_positive(schema: dict) -> None:
    #  The featuriser must refuse a model whose recorded schema version differs
    #  from the one it implements, so the version has to exist and be comparable.
    assert isinstance(schema["schema_version"], int)
    assert schema["schema_version"] >= 1


def test_blocks_tile_the_row_with_no_gap_or_overlap(schema: dict) -> None:
    cursor = 0
    for block in schema["blocks"]:
        assert block["offset"] == cursor, (
            f"block {block['name']!r} starts at {block['offset']} but the previous "
            f"block ended at {cursor} -- a gap leaves a column permanently zero, "
            "an overlap makes two properties share a bit"
        )
        assert block["width"] > 0, f"block {block['name']!r} has zero width"
        cursor += block["width"]

    assert cursor == schema["n_features"]


def test_block_names_are_unique(schema: dict) -> None:
    names = [b["name"] for b in schema["blocks"]]
    assert len(names) == len(set(names)), f"duplicate block names: {names}"


@pytest.mark.parametrize(
    ("block_name", "list_key"),
    [
        ("element", "element_order"),
        ("hybridisation", "hybridisation_order"),
        ("formal_charge", "charge_buckets"),
        ("degree", "degree_buckets"),
        ("num_hs", "hydrogen_buckets"),
    ],
)
def test_one_hot_list_lengths_match_block_widths(
    schema: dict, block_name: str, list_key: str
) -> None:
    # These lists are indexed POSITIONALLY by the featuriser. A reordering without
    # a SCHEMA_VERSION bump silently relabels atoms -- carbon becoming nitrogen as
    # far as the model is concerned.
    width = next(b["width"] for b in schema["blocks"] if b["name"] == block_name)
    assert len(schema[list_key]) == width


def test_unknown_hybridisation_maps_to_sp3(schema: dict) -> None:
    #  DEF-04. `Hybridisation::Unknown` clamps to index 2, so index 2 must be sp3.
    #  The generic `min(width - 1)` clamp put it on sp3d2 -- octahedral, the rarest
    #  geometry in drug-like space -- for every atom whose hybridisation could not
    #  be determined.
    assert schema["hybridisation_order"][2] == "sp3"


def test_max_heavy_atoms_matches_the_exported_graph(schema: dict) -> None:
    #  ADR-03: the atom axis is fixed at 128, which is what makes the graph
    #  exportable. Confirmed against real data in docs/evidence/increment-1: only 8
    #  molecules out of 37,289 exceed it.
    assert schema["max_heavy_atoms"] == 128


def test_charge_buckets_are_sorted_and_centred_on_zero(schema: dict) -> None:
    buckets = schema["charge_buckets"]
    assert buckets == sorted(buckets), "charge buckets must be ordered"
    assert 0 in buckets, "a neutral atom needs a bucket"
    #  Symmetric range, so clamping behaves the same in both directions.
    assert buckets[0] == -buckets[-1]


def test_clamping_behaviour_is_documented_in_the_artefact(schema: dict) -> None:
    #  The Python featuriser must clamp identically to Rust. Shipping the rule
    #  inside the contract means the two implementations are reading the same
    #  sentence rather than two docstrings that drifted.
    assert "clamp" in schema["clamping"].lower()
