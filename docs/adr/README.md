# Architecture Decision Records

An ADR records a decision that **closed off an alternative**. Not every choice
qualifies: the test is whether a competent person could reasonably have chosen
differently, and whether reversing it later would cost real work. Naming a
variable does not qualify. Choosing dense adjacency over sparse scatter does,
because it made PyTorch Geometric unusable for the rest of the project.

## Why these are written now, before most of the code

Because an ADR written after the fact is a rationalisation, and it reads like one.
The value of these seven is that they were reasoned through when the alternatives
were still live — the git history shows them dated before the implementation they
constrain, which is the difference between a design decision and an excuse.

That also makes them useful in the way documentation rarely is: when you are six
weeks in and wondering why `admet-core` has no database access, the answer is
[ADR-02](0002-hexagonal-crate-split.md), and it includes the cost of that choice
as well as the benefit.

## The records

| # | Decision | Status | Affects |
|---|---|---|---|
| [ADR-01](0001-rust-serving-onnx-boundary.md) | Rust serves inference; ONNX is the only Python→Rust artefact | Accepted | everything |
| [ADR-02](0002-hexagonal-crate-split.md) | Five crates; `admet-core` has zero I/O | Accepted | crate layout, test speed, Increment 5 |
| [ADR-03](0003-dense-adjacency-over-sparse-scatter.md) | Padded dense adjacency and `torch.bmm`, not PyG scatter | Accepted | model architecture, 128-atom cap |
| [ADR-04](0004-inchikey-identity-and-cache-key.md) | InChIKey is molecular identity; cache keyed `(inchikey, model_version)` | Accepted | schema, caching, TR-05 |
| [ADR-05](0005-scaffold-split-not-random.md) | Bemis–Murcko scaffold split; both strategies reported | Accepted | every reported metric |
| [ADR-06](0006-tdc-over-moleculenet.md) | TDC ADMET group, twelve endpoints, frozen | Accepted | data pipeline, comparability |
| [ADR-07](0007-native-windows-and-training-layout.md) | Native Windows over WSL2; `training/` over `ml/src/admet_ml/` | Accepted | two documented deviations from the build manual |

## Conventions

- **Filename**: `NNNN-kebab-case-title.md`, numbered in the order decided.
- **Immutable once accepted.** A decision that changes gets a *new* ADR that
  supersedes the old one, and the old one's status becomes `Superseded by
  ADR-NN`. Editing an accepted ADR destroys the only thing it was for — the
  record of what was believed at the time.
- **Statuses**: `Proposed` → `Accepted` → `Superseded by ADR-NN` / `Deprecated`.
- **Every ADR states its cost.** An ADR with no "Consequences (negative)" section
  is advocacy, not a record. If a decision genuinely has no downside it did not
  need an ADR, because nobody would have chosen otherwise.
- Commits that act on a decision carry it in the trailer: `Refs: ADR-03`. That
  makes `git log --oneline --grep="ADR-03"` the list of everything one decision
  caused.

## Template

```markdown
# ADR-NN: Title in the imperative

- **Status**: Proposed | Accepted | Superseded by ADR-NN
- **Date**: YYYY-MM-DD
- **Deciders**: <name>
- **Related**: FR-nn, TR-nn, NFR-nn, R-n

## Context
The forces in play. What is true that makes this a decision rather than an
obvious step. Include the constraint that hurts.

## Decision
One paragraph, active voice, present tense. "We use X."

## Consequences

### Positive
### Negative
### Neutral

## Alternatives considered
One subsection each, with the reason it lost. An alternatives section that
dismisses everything in half a line was not a real evaluation.

## References
```
