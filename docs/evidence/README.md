# Evidence — the artefacts that make a claim checkable

| | |
|---|---|
| **System** | ADMETriage |
| **Document** | What belongs in this directory, in what form, and why it is committed |
| **Status** | Open. Empty at the scaffold tag; fills from Increment 1 onward. |
| **Traces** | [`03-test-plan.md`](../03-test-plan.md) §9 · [`04-traceability.md`](../04-traceability.md) §6 |

## 1. What this directory is for

Some claims cannot be discharged by a test. "The UI visually distinguishes a computed
descriptor from a prediction" (NFR-10) is true or false to a human looking at a
screen; an assertion on a CSS class would prove the class is applied, not that anyone
can tell the difference. "The desktop build runs with the network interface disabled"
(TR-02, FR-24) is a test, but the *proof that it was run on a disabled interface* is a
screenshot of `ipconfig` beside a working prediction.

This directory holds those artefacts. Every file in it exists so that a specific
sentence in the report stops being an assertion.

`.gitignore` has an explicit `!docs/evidence/**` exception, because the general rules
ignore `*.log`, `*.png`-adjacent build output and `*.npz`. That exception is
deliberate: **evidence that is not committed is not evidence.** A screenshot on a
desktop is a memory of having checked.

## 2. What belongs here, and what does not

| Belongs | Does not |
|---|---|
| Screenshots proving a visual requirement (NFR-10, FR-13, FR-20) | Screenshots of code, or of a terminal that could have been pasted as text |
| Terminal captures where the *environment* is the point — `ipconfig` with the adapter down, `docker stats` during a batch, `nproc` inside the constrained container | Ordinary command output that a script in `results/` produces |
| One-off inspection records: `cargo tree` output for NFR-09, the licence sweep, the "no Python in `crates/`" check | Anything regenerable by `just results` — that lives in `results/` |
| A dated note recording a manual check, with the commit it was performed at | A note recording an intention |
| Installer size measurements for NFR-12, with the `ls -l` that produced them | The installer itself — build artefacts do not belong in git |

The line between this directory and `results/` is worth stating plainly: **`results/`
holds numbers a script regenerates; `docs/evidence/` holds observations that cannot be
regenerated because they were made by a person at a point in time.** If a file could
live in `results/`, it should.

## 3. Naming

```
YYYY-MM-DD-<short-slug>-<commit>.<ext>
```

Example: `2026-10-14-nfr10-predicted-vs-computed-a1b2c3d.png`.

Three parts, each earning its place:

- **The date** — evidence has a shelf life. A screenshot of the UI from before a
  redesign proves something about a version that no longer exists, and the date is how
  a reader knows to distrust it.
- **The slug** — names the requirement or check, so `ls` reads as a coverage list.
- **The short commit hash** — the only way to reproduce the state that was observed.
  Without it, "the UI looked like this" is unanchored.

## 4. Every artefact needs a caption file

A screenshot alone is ambiguous: a reader cannot tell what they are supposed to
notice. So each artefact gets a sibling `.md` with the same stem:

```markdown
---
requirement: NFR-10
date: 2026-10-14
commit: a1b2c3d
method: manual inspection, Firefox 142, 1920x1080, default zoom
---

Molecular weight and cLogP render in the "Computed" panel with a solid border and
no confidence marker; the twelve endpoint values render in the "Predicted" panel,
each with its domain-status chip. The two panels are separated by a labelled rule.

What a reader should conclude: a chemist cannot mistake a predicted hERG probability
for a measured descriptor without ignoring both the panel heading and the chip.

Known weakness: this is one screenshot at one viewport. It does not establish that
the distinction survives at mobile width, which is not a requirement.
```

The last two blocks are the ones that make this worth doing. **"What a reader should
conclude"** forces the artefact to have a point. **"Known weakness"** is what
separates evidence from advocacy — an assessor will find the weakness anyway, and
finding it already written down is a different experience from finding it hidden.

## 5. The register

Filled in as artefacts land. A row with a date and no file is a task; a file with no
row is untraced.

| Requirement | Artefact | Date | Inc | Status |
|---|---|---|---|---|
| NFR-09 — no paid API, no network dependency | `cargo tree` sweep + dependency licence list | | 2 | → |
| TR-02 — no Python on the request path | inspection record: no `.py` under `crates/` | | 2 | → |
| NFR-10 — computed vs predicted are visually distinct | screenshot + caption | | 3 | → |
| FR-13 — 2D depiction renders | screenshot of the canvas at three molecule sizes | | 3 | → |
| FR-20 — attribution overlay | screenshot of a worked example beside its atom scores | | 4 | → |
| NFR-02 — 2 vCPU constraint was actually applied | `nproc` inside the container during `TC-P-004` | | 4 | → |
| NFR-12 — peak RSS during a 10,000-row batch | `docker stats` capture | | 4 | → |
| TR-02 / FR-24 — offline desktop | `ipconfig` with the adapter disabled, beside a completed prediction | | 5 | → |
| NFR-12 — installer under 15 MB | `ls -l` on the bundle | | 5 | → |
| Increment 5 exit — clean checkout from written steps only | terminal log of the whole run on a fresh clone | | 5 | → |

Ten rows, and the last is the one most often skipped. A clean-checkout log is the only
artefact that proves the documentation is complete rather than merely present — every
project works on the machine it was built on, and the log is what distinguishes
"documented" from "documented and tried".

## 6. The honesty rule, restated because this is where it is easiest to break

An artefact that shows a failure is committed too. If `TC-P-004` misses the 90-second
target, the capture showing 112 seconds goes in this directory and the number goes in
`results/`, and the report says the target was missed and why.

Deleting an inconvenient artefact and re-running until a good one appears is the one
thing that would make everything else in this directory worthless — because a reader
who suspects selection cannot trust any of it.
