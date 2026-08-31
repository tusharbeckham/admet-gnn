# Git Conventions

> Build Manual chapter 5. The rules that make the repository's history usable as
> evidence rather than as a pile of "wip" commits.

The point of everything below is one property: **a reader should be able to
reconstruct what happened, and why, from `git log` alone**. That property is worth
having for its own sake, and it is also directly graded — the traceability matrix
in [04-traceability.md](04-traceability.md) is *derived* from commit trailers, not
maintained by hand.

---

## 1. Branch model

Manual Table 5.1. Three long-lived branches would be two too many for one person,
so there are two.

| Branch | Lives | Protected | Purpose |
|---|---|---|---|
| `main` | forever | yes | Always builds, always passes CI. Every tag comes from here. |
| `develop` | forever | no | Integration. Where a half-finished increment lives. |
| `feat/<slug>` | days | no | One feature. Merges to `develop`. |
| `fix/<slug>` | hours | no | One defect. Carries its `DEF-nn`. |
| `spike/<slug>` | hours | no | Throwaway experiment. **Expected to be deleted**, not merged. |
| `docs/<slug>` | hours | no | Document-only work. |

`spike/` earning its own prefix is deliberate. Naming a branch as disposable up
front is what makes it psychologically possible to throw away — a branch called
`feat/try-sparse-scatter` accumulates sunk cost, and `spike/sparse-scatter` does
not. The ONNX export spike that produced ADR-03 was exactly this: its value was
the *finding*, and none of its code survives.

### Working solo does not mean working on `main`

The temptation is real and the reason to resist it is concrete: a branch plus a
pull request is what makes the [PR
checklist](../.github/PULL_REQUEST_TEMPLATE.md) fire, and that checklist is the
only reviewer this project has. Committing straight to `main` skips it silently.

```bash
git switch -c feat/smiles-lexer develop
# ... work, commit ...
git push -u origin feat/smiles-lexer
gh pr create --base develop --fill
```

---

## 2. Commit messages

[Conventional Commits](https://www.conventionalcommits.org/), because the format
is machine-parseable and therefore the changelog and the traceability matrix can
be generated instead of written.

```text
<type>(<scope>): <subject>

<body: why, not what>

Refs: FR-04, NFR-06
```

### Types

| Type | Use for | In changelog |
|---|---|---|
| `feat` | New capability a user can observe | yes |
| `fix` | Defect repair. **Always carries a `DEF-nn`** | yes |
| `perf` | Measured speed or memory improvement | yes |
| `refactor` | Behaviour-preserving restructure | no |
| `test` | Tests only | no |
| `docs` | Documentation only | no |
| `build` | Cargo/pnpm/Docker/CI plumbing | no |
| `chore` | Dependency bumps, housekeeping | no |
| `spike` | Experiment whose output is a finding | no |

`perf` has a rule attached: it requires a before-and-after number in the body.
A commit claiming an improvement with no measurement is the exact habit the
standing honesty rule exists to prevent, and it is also how a "5× faster" claim
ends up in a report unchallenged.

### Scopes

One per crate or top-level area, so `git log --oneline --grep="(core)"` is a
useful filter:

```text
core   infer   db   api   cli   web   desktop   train   data   docs   ci
```

### Subject line

- Imperative mood: "add", not "added" or "adds". It reads as an instruction to the
  codebase, which is what a commit is.
- No trailing full stop, lower case after the colon, ≤ 50 characters.
- Say what changed *observably*. `fix(core): reject molecules over 128 atoms` is
  useful; `fix(core): fix bug` is a line that will be read twenty times and help
  nobody.

### Body

The body answers **why**. The diff already shows what. The most valuable body is
the one that records the alternative you rejected — six weeks later that is the
information you cannot recover from the code.

```text
perf(infer): reuse the input tensor allocation across a batch

Allocating x[64,128,33] per micro-batch showed up as 31% of wall time in
the criterion sweep. Reusing one buffer and zeroing the padded rows drops
p95 from 41ms to 27ms at batch 64 (bench: admet-core/infer_batch).

Rejected: an arena allocator. The lifetime plumbing reaches into
Engine::run's signature and the win over a reused Vec was under 2%.

Refs: NFR-01, NFR-02
```

---

## 3. The `Refs:` trailer

**Every commit carries one.** This is the single convention that most repays the
effort, because it turns the requirement identifiers into a queryable index.

```bash
git log --oneline --grep="FR-14"      # everything that touched this requirement
git log --oneline --grep="DEF-"       # the entire defect history, in order
git log --oneline --grep="ADR-03"     # every consequence of one decision
git log --oneline --grep="NFR-01"     # every change made for latency
```

Valid prefixes, all defined in [01-srs.md](01-srs.md):

| Prefix | Range | Meaning |
|---|---|---|
| `FR-` | 01–24 | Functional requirement |
| `TR-` | 01–12 | Technical requirement |
| `NFR-` | 01–12 | Non-functional requirement |
| `UC-` | 01–08 | Use case |
| `ADR-` | 01–07 | Architecture decision ([adr/](adr/)) |
| `DEF-` | 01–… | Defect ([03-test-plan.md](03-test-plan.md) §defect log) |
| `G` | G1–G8 | Project goal — **no hyphen**, matching [requirements.md](../requirements.md) |
| `R` | R1–R10 | Risk — **no hyphen**, same reason |

If a commit genuinely refs nothing — a typo in a comment — write `Refs: none`
rather than omitting the trailer. An explicit `none` is a decision; an absent
trailer is indistinguishable from forgetting.

### Why not GitHub issues instead

Issues are fine, and `Closes #14` still works. But an issue number is meaningful
only inside this repository's issue tracker, whereas `FR-14` is meaningful in the
SRS, the design document, the test plan, the traceability matrix and the final
report. The identifiers that appear in the graded documents are the ones worth
putting in commits.

---

## 4. Tags

Annotated, never lightweight — a lightweight tag carries no author, date or
message, so it cannot say what it marks.

```bash
git tag -a v0.1.0-increment1 -m "Increment 1: trained model, ONNX export verified"
git push origin v0.1.0-increment1
```

| Tag | Marks |
|---|---|
| `v0.0.1-scaffold` | Repository builds clean, CI green, documents skeletoned |
| `v0.1.0-increment1` | Twelve endpoints trained; `model.onnx` exported and verified |
| `v0.2.0-increment2` | SMILES parser, featuriser, `/predict` end to end |
| `v0.3.0-increment3` | Web UI, authentication, batch screening |
| `v0.4.0-increment4` | Explainability, similarity search, performance work |
| `v1.0.0` | Desktop build; everything in the SRS satisfied or explicitly deferred |

Attach the `model.onnx` for that increment to the corresponding GitHub Release.
That is how a result stays reproducible without committing a 40 MB binary — see
[models/README.md](../models/README.md).

---

## 5. What never enters history

The [pre-commit hook](../.githooks/pre-commit) enforces the first item and warns
about the rest.

- **Credentials.** Any connection string with a password, any token, any `.pem`.
  Once committed, rewriting history is the *second* thing you do; rotating the
  credential is the first, because it must be assumed compromised the moment it
  is pushed.
- **Datasets.** `.csv.gz`, `.parquet`, `.npz`, `.sdf`. Regenerable by
  `just data-download`. Git stores every version in full.
- **Model weights.** `.onnx`, `.pt`. Releases, not git. The committed
  `.sha256` is what ties a prediction to an artefact.
- **`target/`, `node_modules/`, `.venv*/`.** Machine-specific and enormous.

`Cargo.lock` **is** committed. The workspace produces binaries, not a published
library, and a lock file is the difference between "it worked in CI" and "it
worked in CI with the dependency versions that existed that Tuesday".

---

## 6. Installing the hook

```bash
just hooks
```

which is `git config core.hooksPath .githooks`. Two consequences worth knowing:

- The hook is **versioned**. A hook living in `.git/hooks` is invisible to git,
  so it drifts from CI and nobody notices when it stops running.
- `core.hooksPath` is per-clone local config, so it must be run once in every
  clone. `just setup` depends on `just hooks` for that reason.

Bypass with `git commit --no-verify` when you genuinely need a work-in-progress
commit on a branch. If you reach for it more than about once a week, the hook is
wrong — fix the hook rather than training yourself around it.

---

## 7. Rewriting history: the one rule

Rebase freely on a branch nobody has pulled. Never rewrite `main` or `develop`.

```bash
git rebase -i develop        # tidy your own branch before opening the PR
```

Squashing eight "wip" commits into one `feat(core): add SMILES lexer` is worth
doing, and it is worth doing *before* the pull request rather than at merge time,
because the squashed message is the one that ends up in `git log` forever.

The exception to squashing: if two of those commits are genuinely separate
findings — a spike that failed and the approach that worked — keep them both. The
failed attempt is the more interesting half of the history, and it is the half
that makes an ADR credible.
