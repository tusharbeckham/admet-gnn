---
week: NN
dates: YYYY-MM-DD to YYYY-MM-DD
increment: 0 | 1 | 2 | 3 | 4 | 5
planned_hours:
actual_hours:
---

# Week NN — <three words that name the week>

> Copy this file to `week-NN.md` and fill it in **at the end of the week, before
> the weekend**. Written on Monday about the previous week it is fiction; the
> details that make it useful — which error message, how long the wrong path took —
> survive about three days.

## 1. What was planned

Copied from last week's §7, unedited. If it was wrong, that is the finding, and
editing it away deletes the finding.

- [ ]
- [ ]

## 2. What actually shipped

Only things that exist in the repository. A commit hash or a file path per line; if
neither can be given, it did not ship.

| What | Where | Commit |
|---|---|---|
| | | |

## 3. Requirements touched

IDs from [`requirements.md`](../../requirements.md), matching the `Refs:` trailers
on this week's commits. This column is what lets
[`04-traceability.md`](../04-traceability.md) be updated from the journal rather
than from memory.

| ID | Moved from → to | Test now exists? |
|---|---|---|
| | `→` → ✅ stub | no |

## 4. Decisions

A decision is a point where two options were live and one was chosen. Not "used
Axum" — that was decided in ADR-01. This is for the ones made *this week*.

| Decision | Alternative rejected | Why | Needs an ADR? |
|---|---|---|---|
| | | | no |

**Rule:** if the answer in the last column is "yes", write the ADR this week. An
ADR written in week 14 about a week-6 decision is a reconstruction, and it reads
like one.

## 5. Defects

Every defect gets a `DEF-nn` in [`03-test-plan.md`](../03-test-plan.md) §10.2 **when
found**, not when fixed. Record it here too, because this file is where the cost is
visible.

| DEF | Sev | Symptom | Root cause | Which test would have caught it | Hours lost |
|---|---|---|---|---|---|
| | | | | | |

The fifth column is the one that pays for the table. If the honest answer is "no
test in the current plan would have caught this", the plan has a hole and it gets
one new row in §5 of the test plan.

## 6. Numbers measured this week

Only measurements. A target belongs in `requirements.md`; a guess belongs nowhere.

| What | Value | Produced by | Landed in |
|---|---|---|---|
| | | `just …` | `results/…` |

If a number in this table contradicts a target, say so in one sentence here rather
than adjusting anything. A missed target recorded as a miss is a result; a target
quietly moved is the beginning of a report nobody can trust.

## 7. Next week

Becomes next week's §1 verbatim, so write it as tasks with an observable end state,
not as areas of activity.

- [ ]
- [ ]

## 8. Increment exit criteria — standing checklist

Copied from [`03-test-plan.md`](../03-test-plan.md) §9 for the increment named in
the front matter. Carried forward unchanged each week, ticked as they close, so the
distance to the tag is visible every Friday instead of discovered at the end.

- [ ]
- [ ]

## 9. The honest paragraph

One paragraph, written last, answering: **what went wrong this week, and what does
it say about the plan?**

This is the only section of the journal that ends up quoted in the report's
evaluation chapter, and it is worth more than the other eight combined — because a
retrospective assembled in week 15 from a clean commit log will say the project
went well, and no project goes well. The specific afternoon lost to a CRLF-mangled
binary fixture is the thing an assessor recognises as real.

Write it even in a week where nothing went wrong. "Nothing went wrong and I am
suspicious of that" is itself a finding.
