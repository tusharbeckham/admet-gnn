# Pull request

<!--
  Manual Table 5.2. This template exists because a solo project has no second
  reviewer, so the checklist IS the reviewer. Filling it in honestly is the whole
  value; ticking every box reflexively converts it into decoration.

  Anything you cannot tick, delete the box and write one line saying why. "Not
  applicable: docs only" is a complete answer. A silently ticked box that was not
  true is the failure mode this template is trying to prevent.
-->

## What changed

<!-- Two or three sentences. What a reader needs to know before the diff. -->

## Why

<!--
  The problem, not the solution. "Predictions for large molecules were silently
  truncated" tells a reader what to look for; "added a length check" does not.
-->

## Refs

<!--
  REQUIRED. At least one identifier, comma separated. This is what makes the
  traceability matrix (docs/04-traceability.md) derivable from git rather than
  maintained by hand:

      git log --oneline --grep="FR-14"     every commit touching that requirement
      git log --oneline --grep="DEF-"      the whole defect history

  Valid prefixes: FR- TR- NFR- UC- ADR- DEF- G- R-
-->

Refs:

---

## Self-review checklist

### Correctness
- [ ] I have read my own diff line by line, not just the summary
- [ ] Every new branch has a test that fails without the change
- [ ] Error paths are tested, not only the happy path
- [ ] No `unwrap()` / `expect()` / `panic!()` on a request path (tests are fine)
- [ ] No `todo!()` or `unimplemented!()` reachable at run time

### Gates
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo nextest run --workspace` green
- [ ] `just check` green locally (so CI will be too)

### The two that are specific to this project
- [ ] **Featuriser parity.** If this touches the 33-feature layout in *either*
      language, `models/feature_schema.json` was regenerated and the parity
      fixture still agrees within 1e-4. A silent disagreement here is a wrong
      prediction, not a crash — see TR-03, risk R3.
- [ ] **Targets vs measurements.** Every number added to a document or a comment
      is either produced by a script in this repo, or is labelled *target*. No
      figure is asserted from memory.

### Security
- [ ] No credential, token, key or connection string in the diff (including tests
      and fixtures)
- [ ] New SQL is parameterised — no string interpolation into a query
- [ ] New user input is validated at the boundary, with a bound on its size
- [ ] A new route's authentication requirement is explicit, and if it has none
      that is stated in the PR body rather than left for a reader to notice
- [ ] `Debug`/log output for any new type carrying a secret is redacted

### Documentation
- [ ] Public items have doc comments that say *why*, not what the signature
      already says
- [ ] A decision that closes off an alternative got an ADR, or amended one
- [ ] `docs/04-traceability.md` updated if an identifier's status changed
- [ ] The weekly journal entry records what actually happened, including what did
      not work

## How I tested this

<!--
  Commands and outcomes, not intentions. "Ran the suite" is not evidence;
  "cargo nextest run --workspace: 143 passed, 2 skipped" is.
-->

```text

```

## Known gaps

<!--
  What is deliberately not done here, and where it is tracked. An empty section
  is suspicious in anything larger than a typo fix.
-->
