# Journal — fifteen weekly entries

| | |
|---|---|
| **System** | ADMETriage |
| **Document** | Development journal: index, schedule, and the rules for keeping it |
| **Status** | Open. Template written; `week-01.md` is the first task of week 1. |
| **Traces** | [`implementation.md`](../../implementation.md) §3 · [`03-test-plan.md`](../03-test-plan.md) §10 |

## 1. Why keep this

The report has an evaluation chapter, and that chapter is graded on whether it
contains anything a reader could not have guessed. A retrospective assembled in week
15 from a clean commit log always says the same three things: the project went
broadly to plan, Rust had a learning curve, and more time for testing would have
helped. None of that is worth reading.

What is worth reading is the afternoon lost because a `.f32` fixture was
CRLF-mangled on Windows and the parity test failed for a reason nobody would think
to look for. That detail survives about three days in memory. This directory is the
mechanism for catching it.

Second use, more mundane and more immediate: **§3 of each entry is what keeps
[`04-traceability.md`](../04-traceability.md) true.** Updating a matrix from a week's
notes takes two minutes. Updating it from six weeks of memory does not happen.

## 2. How to keep it

- **One file per week: `week-01.md` … `week-15.md`.** Copy
  [`WEEK-TEMPLATE.md`](WEEK-TEMPLATE.md); do not restructure it, because the value of
  a template is that section 5 is always defects.
- **Write it Friday**, before stopping. Not Monday about last week.
- **Commit it in its own commit**, typed `docs`, so `git log -- docs/journal` is the
  project's narrative in order.
- **Never edit an earlier entry** except to fix a typo. If week 4's plan turned out
  wrong, week 5 says so. Rewriting week 4 destroys the only evidence that the plan
  changed, which is the interesting part.
- **A missed week gets a stub** saying what happened instead — illness, an exam, a
  week lost to a rabbit hole. A gap in the sequence with no explanation reads as
  concealment; a one-line stub reads as a project run by a person.

## 3. The schedule this journal tracks

From [`implementation.md`](../../implementation.md) §3. The `Entry` column is the
file that should exist by the end of that week.

| Week | Focus | Increment | Entry | Tag due |
|---|---|---|---|---|
| 1 | Toolchain, **ONNX spike**, scaffold, CI skeleton | 0 | `week-01.md` | `v0.0.1-scaffold` |
| 2 | Scaffold finished; requirements analysis begins | 0 | `week-02.md` | |
| 3 | Requirements analysis; SRS, use cases, context diagram | 0 | `week-03.md` | |
| 4 | Design: ER model, class diagram, ADRs; **Increment 1 starts** | 1 | `week-04.md` | |
| 5 | Data pipeline, features, dense GIN, first training runs | 1 | `week-05.md` | |
| 6 | Metrics on both splits, golden fixture; **Increment 2 starts** | 1→2 | `week-06.md` | `v0.1.0-model` |
| 7 | `admet-core`: parser, graph, features, fingerprint | 2 | `week-07.md` | |
| 8 | `admet-infer`, `admet-db`, `admet-api`; parity green | 2→3 | `week-08.md` | `v0.2.0-api` |
| 9 | SvelteKit workspace, single-molecule report | 3 | `week-09.md` | |
| 10 | Projects, auth, NFR-10 evidence; **Increment 4 starts** | 3→4 | `week-10.md` | `v0.3.0-web` |
| 11 | Batch pipeline, streaming progress, triage ranking | 4 | `week-11.md` | |
| 12 | Attribution, export; **Increment 5 starts** | 4→5 | `week-12.md` | `v0.4.0-batch` |
| 13 | Tauri desktop, Docker, release automation | 5 | `week-13.md` | `v0.5.0-desktop` |
| 14 | System and acceptance testing; traceability matrix closed | — | `week-14.md` | |
| 15 | Deployment, documentation freeze, report | — | `week-15.md` | `v1.0.0` |

Two weeks are load-bearing and worth naming:

- **Week 1 is the spike week.** If `just spike` fails, the fallback (a Python
  inference sidecar behind the same HTTP contract) is decided in week 1 and costs a
  day. Discovered in week 8, the same fact costs the architecture.
- **Week 14 is not slack.** It is when the traceability matrix stops being a list of
  intentions, and it is the only week whose output is entirely documentation. A
  project that runs late will try to spend it, and the tag schedule above is what
  makes that visible in advance.

## 4. Index

Filled in as entries land. The `Honest paragraph` column is a three-word summary of
§9, so this table can be skimmed for the report's evaluation chapter.

| Entry | Week | Shipped | Defects found | Honest paragraph |
|---|---|---|---|---|
| — | — | — | — | *first entry due end of week 1* |

## 5. What this is not

Not a diary — nobody is grading how the week felt. Not a substitute for the defect
log: `DEF-nn` lives in [`03-test-plan.md`](../03-test-plan.md) §10.2, which is the
single authoritative list, and the journal records the *cost* of each. Not a place
for numbers: measurements live in `results/`, and a figure typed into a journal entry
is a figure that will be quoted later without its provenance.
