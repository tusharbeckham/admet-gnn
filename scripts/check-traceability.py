#!/usr/bin/env python
"""Check that docs/04-traceability.md is telling the truth.

`docs/04-traceability.md` §1 says the matrix is "hand-maintained but
machine-checked" and spells out the check as four shell greps. `README.md` calls it
"machine-checkable". Nothing ran it. This script is that check.

Standard library only, so it runs on a fresh clone with no environment.

---------------------------------------------------------------------------
 What this gate does and does not enforce
---------------------------------------------------------------------------
It does NOT demand a complete matrix. At the scaffold tag not one test named in the
document exists, and the document says so plainly in §9 — a matrix full of `→` is
honest.

What it enforces is that the document's **claims match reality**:

  * a requirement ID cited in the matrix must exist in the register
    (citing a requirement nobody wrote is a lie);
  * a test ID that exists in the code must be cited in the matrix
    (an untraced test is either dead weight or an undocumented requirement);
  * the counts written into §9 must reproduce when recomputed.

That last one is the point. section 9 says "Counted by the scripts in §1 rather than by
reading … re-run the greps and they either reproduce or this section is stale." So
the failure mode being prevented is not an incomplete matrix — it is a matrix whose
summary was true in August and quietly stopped being true in October.

Consequence, deliberately: the day someone writes `TC-U-090`, this script fails
until §9 is updated. That is the intended behaviour, not friction to route around.

Exit codes:  0 = consistent   1 = inconsistent   2 = could not run
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
MATRIX = REPO_ROOT / "docs" / "04-traceability.md"
REGISTER = REPO_ROOT / "requirements.md"
CODE_DIRS = ("crates", "training")

REQ_ID = re.compile(r"\b(?:FR|TR|NFR|UC)-\d+\b")
TC_ID = re.compile(r"\bTC-(?:U|I|SYS|P|S)-\d+\b")

#  Source files only. Scanning the matrix's own prose for TC ids, or a build
#  artefact under target/, would make the comparison circular.
CODE_SUFFIXES = {".rs", ".py", ".sql", ".ts", ".svelte"}


class Report:
    def __init__(self) -> None:
        self.failures: list[str] = []
        self.notes: list[str] = []

    def fail(self, msg: str) -> None:
        self.failures.append(msg)

    def note(self, msg: str) -> None:
        self.notes.append(msg)


def ids_in(path: Path, pattern: re.Pattern[str]) -> set[str]:
    return set(pattern.findall(path.read_text(encoding="utf-8", errors="replace")))


def ids_in_code(pattern: re.Pattern[str]) -> dict[str, list[str]]:
    """Map each id found in source to the files citing it."""
    found: dict[str, list[str]] = {}
    for directory in CODE_DIRS:
        root = REPO_ROOT / directory
        if not root.is_dir():
            continue
        for file in root.rglob("*"):
            if file.suffix not in CODE_SUFFIXES or not file.is_file():
                continue
            #  Skip virtualenvs and caches that may live under training/.
            parts = set(file.parts)
            if parts & {"__pycache__", "site-packages", "target", "node_modules"}:
                continue
            text = file.read_text(encoding="utf-8", errors="replace")
            for match in pattern.findall(text):
                found.setdefault(match, []).append(
                    str(file.relative_to(REPO_ROOT)).replace("\\", "/")
                )
    return found


def claimed_count(matrix_text: str, label: str) -> int | None:
    """Pull a number out of a §9 table row by its `Measure` label.

    The cell may be bolded (`**56**`) and may carry a trailing "of 36", so the
    first integer in the second column is taken.
    """
    for line in matrix_text.splitlines():
        if not line.startswith("|") or label not in line:
            continue
        cells = [c.strip() for c in line.strip("|").split("|")]
        if len(cells) < 2:
            continue
        number = re.search(r"\d+", cells[1])
        if number:
            return int(number.group())
    return None


def main() -> int:
    for path in (MATRIX, REGISTER):
        if not path.is_file():
            print(f"FATAL cannot find {path}", file=sys.stderr)
            return 2

    matrix_text = MATRIX.read_text(encoding="utf-8", errors="replace")
    report = Report()

    # --- 1. requirement IDs -------------------------------------------------
    matrix_reqs = ids_in(MATRIX, REQ_ID)
    register_reqs = ids_in(REGISTER, REQ_ID)

    cited_but_unregistered = sorted(matrix_reqs - register_reqs)
    registered_but_uncited = sorted(register_reqs - matrix_reqs)

    if cited_but_unregistered:
        report.fail(
            "requirement IDs cited in the matrix but absent from requirements.md "
            f"(citing a requirement nobody wrote): {', '.join(cited_but_unregistered)}"
        )
    if registered_but_uncited:
        report.fail(
            "requirement IDs in the register but never cited in the matrix "
            f"(an untraced requirement): {', '.join(registered_but_uncited)}"
        )

    # --- 2. test IDs --------------------------------------------------------
    matrix_tcs = ids_in(MATRIX, TC_ID)
    code_tcs = ids_in_code(TC_ID)

    in_code_uncited = sorted(set(code_tcs) - matrix_tcs)
    if in_code_uncited:
        report.fail(
            "test IDs present in code but not cited in the matrix "
            f"(an untraced test): {', '.join(in_code_uncited)}"
        )

    #  The other direction is NOT a failure at this stage. The matrix openly
    #  documents that every Test cell is still a `→`; failing on it would mean the
    #  gate could not be green until the project was finished, and a gate that
    #  cannot be green gets disabled.
    cited_not_yet_written = sorted(matrix_tcs - set(code_tcs))
    if cited_not_yet_written:
        report.note(
            f"{len(cited_not_yet_written)} test IDs are cited in the matrix but do "
            "not exist yet -- expected while the Test column is still an arrow"
        )

    # --- 3. do §9's counts reproduce? --------------------------------------
    #  A label that no longer matches is a FAILURE, not a skip. Renaming a row used
    #  to silently drop its check -- which happened while writing this script:
    #  "Distinct `TC-` IDs cited here" became "Distinct literal `TC-` tokens
    #  appearing here", the verified count fell from 4 to 3, and the run still said
    #  OK. A check that can quietly stop running is the same failure as DEF-09 and
    #  DEF-15, and it does not get to happen a third time.
    expected_claims: list[tuple[str, str, int]] = [
        (
            "Requirement IDs in the register",
            "requirement IDs in the register",
            len(register_reqs),
        ),
        (
            "cited in this matrix",
            "requirement IDs cited in the matrix",
            len(matrix_reqs),
        ),
        (
            "Distinct literal `TC-` tokens appearing here",
            "distinct literal TC- tokens in the matrix",
            len(matrix_tcs),
        ),
        (
            "that exist in `crates/` or `training/`",
            "TC- IDs that exist in code",
            len(code_tcs),
        ),
    ]

    checks = 0
    for label, what, actual in expected_claims:
        claimed = claimed_count(matrix_text, label)
        if claimed is None:
            report.fail(
                f"section 9 has no row matching {label!r}, so the claim about "
                f"{what} is no longer being checked. Restore the row or update this "
                "script -- do not leave the check silently disabled."
            )
            continue
        checks += 1
        if claimed != actual:
            report.fail(
                f"section 9 claims {claimed} {what}, but recounting finds {actual} -- "
                "the gap summary is stale"
            )

    # --- output -------------------------------------------------------------
    print("traceability check")
    print("-" * 64)
    print(f"  requirement IDs registered      {len(register_reqs)}")
    print(f"  requirement IDs cited in matrix {len(matrix_reqs)}")
    print(f"  TC- IDs cited in matrix         {len(matrix_tcs)}")
    print(f"  TC- IDs found in code           {len(code_tcs)}")
    print(f"  section 9 claims verified        {checks}")
    print("-" * 64)

    for note in report.notes:
        print(f"  note  {note}")

    if report.failures:
        print()
        for failure in report.failures:
            print(f"  FAIL  {failure}")
        print(
            "\ntraceability FAILED -- the matrix and the repository disagree.\n"
            "Fix whichever one is wrong; do not silence this check.\n"
        )
        return 1

    print("\ntraceability OK -- every claim in the matrix reproduces.\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
