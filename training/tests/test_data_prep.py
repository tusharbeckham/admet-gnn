"""Tests for the two data-prep modules Increment 1 reuses unchanged.

`scaffold_split.py` and `clean.py` are the only Python modules the plan carries
forward from the MoleculeNet prototype rather than rewriting for TDC. That makes
them the highest-value Python tests in the repository: everything else will be
written fresh with tests alongside it, but these two are inherited, and inherited
code is exactly what nobody re-reads.

`scaffold_split.py` shipped with a self-test in a `__main__` block. A `__main__`
block is not a test -- it runs only when someone remembers to run that file
directly, which is never, and CI cannot see it. These assertions are the same
intent wired into a harness that actually executes.
"""

from __future__ import annotations

import sys
from pathlib import Path

import pandas as pd
import pytest

#  The training tree is not an installed package, so tests reach it by path
#  rather than by import name. Anchored on __file__ so it works from any CWD.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from data.clean import canonicalize, inchikey
from data.scaffold_split import get_scaffold, scaffold_split

# --- scaffolds ---------------------------------------------------------------


def test_benzene_and_toluene_share_a_scaffold() -> None:
    """The load-bearing property of a scaffold split.

    Toluene is benzene with a methyl group. Bemis-Murcko strips side chains, so
    both reduce to the same ring system and MUST land in the same split. If they
    can be separated, the split is measuring memorisation of a ring system the
    model has already seen, and every generalisation number in the report is
    inflated.
    """
    assert get_scaffold("c1ccccc1") == get_scaffold("Cc1ccccc1")


def test_different_ring_systems_do_not_share_a_scaffold() -> None:
    # The negative half. A scaffold function that returned a constant would pass
    # the test above and destroy the split; this is what rules that out.
    assert get_scaffold("c1ccccc1") != get_scaffold("C1CCNCC1")


def test_acyclic_molecule_yields_an_empty_scaffold() -> None:
    # Ethanol has no ring system. Bemis-Murcko returns the empty string rather
    # than failing, and every acyclic molecule therefore shares one bucket --
    # worth pinning, because it is surprising the first time it is noticed.
    assert get_scaffold("CCO") == ""


# --- splitting ---------------------------------------------------------------


def _frame() -> pd.DataFrame:
    #  Four distinct scaffolds, deliberately uneven, plus a duplicate scaffold
    #  (benzene/toluene) so group co-location is observable in the output.
    smiles = [
        "c1ccccc1",
        "Cc1ccccc1",
        "CCc1ccccc1",
        "C1CCNCC1",
        "C1CCNCC1CC",
        "c1ccc2ccccc2c1",
        "CCO",
        "CCCO",
        "c1ccncc1",
        "Cc1ccncc1",
    ]
    return pd.DataFrame({"canonical_smiles": smiles, "y": range(len(smiles))})


def test_split_partitions_every_row_exactly_once() -> None:
    frame = _frame()
    train, val, test = scaffold_split(
        frame, frac_train=0.6, frac_val=0.2, frac_test=0.2
    )

    total = len(train) + len(val) + len(test)
    assert total == 10, "rows were lost or duplicated by the split"

    #  Partition is checked on CONTENT, not on index, because `scaffold_split`
    #  ends with `reset_index(drop=True)` -- each returned frame is numbered from
    #  zero and the original row numbers are gone.
    #
    #  That is pinned here rather than treated as a bug: for training the frames
    #  are consumed whole, so contiguous indices are convenient. It is worth
    #  knowing, though, because it means a leaked scaffold cannot be traced back
    #  to an input line number from the split output alone. If Increment 1 needs
    #  that provenance, carry an explicit `row_index` column -- do not assume the
    #  index survives.
    combined = sorted(
        [
            *train["canonical_smiles"],
            *val["canonical_smiles"],
            *test["canonical_smiles"],
        ]
    )
    assert combined == sorted(frame["canonical_smiles"])

    for part in (train, val, test):
        assert list(part.index) == list(range(len(part))), (
            "frames are reindexed from zero"
        )


def test_no_scaffold_group_spans_two_splits() -> None:
    train, val, test = scaffold_split(
        _frame(), frac_train=0.6, frac_val=0.2, frac_test=0.2
    )

    where: dict[str, set[str]] = {}
    for name, part in (("train", train), ("val", val), ("test", test)):
        for smi in part["canonical_smiles"]:
            where.setdefault(get_scaffold(smi), set()).add(name)

    leaked = {scaffold: splits for scaffold, splits in where.items() if len(splits) > 1}
    assert not leaked, f"scaffold groups split across partitions: {leaked}"


def test_split_is_deterministic_for_a_fixed_seed() -> None:
    # Reproducibility is a requirement, not a nicety: an irreproducible split
    # means a reported metric cannot be regenerated, and the report is then
    # unfalsifiable.
    a = scaffold_split(_frame(), seed=7)
    b = scaffold_split(_frame(), seed=7)
    for left, right in zip(a, b):
        assert list(left["canonical_smiles"]) == list(right["canonical_smiles"])


def test_fractions_must_sum_to_one() -> None:
    with pytest.raises(AssertionError):
        scaffold_split(_frame(), frac_train=0.8, frac_val=0.8, frac_test=0.8)


# --- canonicalisation and identity ------------------------------------------


def test_equivalent_smiles_canonicalise_identically() -> None:
    # The same molecule written two ways. Without canonicalisation these are two
    # rows in the database and two cache entries for one compound.
    assert canonicalize("C1=CC=CC=C1") == canonicalize("c1ccccc1")


def test_invalid_smiles_returns_none_rather_than_raising() -> None:
    # The contract the cleaning pipeline depends on: unparseable input is
    # reported, not fatal, so one bad row cannot abort a 5,000-row import.
    assert canonicalize("this is not a molecule") is None


def test_inchikey_is_27_characters_and_stable() -> None:
    key = inchikey("c1ccccc1")
    assert key is not None
    #  ADR-04 keys the prediction cache on (inchikey, model_version). A key of
    #  unexpected shape would silently change cache behaviour rather than fail.
    assert len(key) == 27, f"InChIKey must be 27 chars, got {len(key)}: {key}"
    assert key == inchikey("C1=CC=CC=C1"), "identity must not depend on SMILES spelling"


def test_inchikey_of_invalid_smiles_is_none() -> None:
    assert inchikey("not a molecule") is None
