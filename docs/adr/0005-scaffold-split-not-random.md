# ADR-05: Split by Bemis–Murcko scaffold, and report both split strategies

- **Status**: Accepted
- **Date**: 2026-08-16
- **Deciders**: tusharbeckham
- **Related**: FR-10, FR-11, TR-12, NFR-03, NFR-07, G5, G8, R4

## Context

Every accuracy figure this project reports depends on how the data was split. That
is not a detail of the evaluation; it is the difference between a number that means
something and a number that does not.

A random split of a molecular dataset leaks. Medicinal chemistry datasets are built
from **series**: a lead compound and forty analogues differing by a methyl here, a
fluorine there. A random split puts some analogues in train and their siblings in
test. The model then scores well by recognising the scaffold it has already seen,
which is a lookup, not a prediction.

The number this inflates is not marginal. Published comparisons on ADMET endpoints
routinely show random-split AUC 0.10–0.15 higher than scaffold-split AUC on the same
data and model. A model reported at 0.90 random / 0.76 scaffold is a model that
will behave like 0.76 on a chemist's novel series — and the chemist's novel series
is the only case that matters, because they already know about the compounds they
have made.

There is a second force pulling the other way, and it is real rather than an
excuse: seven of the twelve endpoints have under 1,300 rows, and `hERG` has 648. A
scaffold split on 648 rows produces a test set that is both small and structurally
narrow, so the variance on the reported metric is large. Making the evaluation
honest also makes it noisy.

## Decision

**The authoritative split is Bemis–Murcko scaffold split.** Implemented in
[`training/data/scaffold_split.py`](../../training/data/scaffold_split.py): compute
each molecule's Bemis–Murcko framework, group by framework, then greedily bin-pack
whole groups into train/valid/test targets (largest group first, seeded shuffle for
tie-breaking). No scaffold group is ever divided across a boundary.

**Both strategies are reported, side by side, for every endpoint.** Random split is
computed and published too — not as the headline, but in the same table.

**Five seeds, mean ± standard deviation.** TDC prescribes five split seeds; a single
run is not a result.

## Consequences

### Positive

- **The headline numbers are the ones that predict field behaviour.** A scaffold-split
  metric answers "how will this do on chemistry it has not seen", which is the
  question a triage tool exists to answer.
- **Publishing both is stronger than publishing only the honest one.** The *gap*
  between random and scaffold is itself a finding: it quantifies how much of the
  model's apparent skill is scaffold memorisation. A report that shows a 0.13 gap
  and discusses it is more credible than one that shows only 0.76, because it
  demonstrates the author knew what they were measuring. It also pre-empts the
  obvious viva question rather than inviting it.
- **Comparable to the literature.** TDC's leaderboards use scaffold splits, so
  these numbers sit next to published ones without an asterisk.
- **Deterministic and reproducible.** Seeded, and the grouping is reproducible in
  Rust — which matters because the domain-applicability check (FR-10) needs to know
  whether a query molecule's scaffold was in the training set, and that requires
  Rust and Python to agree on what the scaffold *is*.
- **It surfaces the domain-of-applicability problem instead of hiding it.** A model
  evaluated on unseen scaffolds visibly fails on some of them, which is the evidence
  that motivates reporting an applicability domain at all (G5) rather than a bare
  number.

### Negative

- **Every reported metric gets worse.** Expect 0.10–0.15 AUC lower than a random
  split. This has to be stated in the report as a deliberate methodological choice,
  because a reader comparing against a random-split paper will otherwise conclude
  the model is weak.
- **Small endpoints get noisy.** On `hERG` (648 rows) a scaffold-split test set is
  perhaps 65 molecules from a handful of frameworks. The standard deviation across
  five seeds will be wide, and reporting a mean without it would be misleading. The
  honest presentation is mean ± sd, and accepting that some confidence intervals
  overlap.
- **Splits are unbalanced.** Greedy bin-packing of whole groups cannot hit 80/10/10
  exactly, and when one scaffold group is 8 % of the dataset it cannot come close.
  Class balance in the test set is also not controllable, so a binary endpoint can
  end up with a test set at a different positive rate than train — which has to be
  accounted for when reading AUC.
- **Bemis–Murcko is one definition of "same scaffold", not the definition.** It keeps
  ring systems and linkers, discards side chains. Two molecules with the same
  framework can differ substantially in properties, and two with different
  frameworks can be near-identical (a ring-opened analogue). Generic-framework and
  fingerprint-cluster splits partition differently. The choice is defensible and
  standard; it is not objectively correct.
- **The scaffold definition becomes a parity surface.** Rust must reproduce the same
  grouping for FR-10. Smaller than the featuriser surface, but it is one more place
  two implementations must agree.

### Neutral

- TDC's own `get_split(method='scaffold')` is used as a **cross-check only**. The
  authoritative split is this repo's, because the Rust side has to reproduce the
  exact grouping and depending on a library's internal behaviour for that is
  fragile. A large disagreement between the two is worth investigating; a small one
  is expected, since the tie-breaking differs.
- Multi-task training means the split must be consistent *across* endpoints for
  molecules that appear in several — otherwise a compound is in train for one
  endpoint and test for another, which is leakage through the shared trunk.

## Alternatives considered

### Random split

Rejected. It measures interpolation within known chemical series and reports it as
generalisation. It is the single most common way an ADMET paper overstates its
result.

### Random split for training decisions, scaffold split for the final report

Superficially reasonable — use the easy split for fast iteration. Rejected because
it means hyperparameters, early stopping and model selection are all tuned against
a leaky signal, and the final scaffold number then measures a model optimised for
the wrong objective. If the split is going to be scaffold at the end, it has to be
scaffold throughout.

### Time-based split

The gold standard where it is available: train on compounds synthesised before a
date, test on after. It is the closest analogue to real prospective use. Rejected
because TDC does not provide synthesis dates for these endpoints. Worth naming in
the report as the split that would be better if the metadata existed.

### Fingerprint-cluster split (Butina, Taylor)

Cluster by Tanimoto similarity and hold out whole clusters. Arguably stricter than
scaffold split, since it catches ring-opened analogues that Bemis–Murcko treats as
unrelated. Rejected as the primary strategy on comparability: TDC's leaderboards
are scaffold-based, and using a different partition would make every external
comparison require an explanation. A defensible extension if there is time in
Increment 4.

### Scaffold split with stratification to balance classes

Would fix the unbalanced-test-set problem. Rejected because balancing requires
splitting scaffold groups, which reintroduces exactly the leakage the split exists
to prevent. The unbalanced test set is the lesser problem, and it can be handled at
reporting time by using AUC and reporting the base rate.

## References

- Bemis & Murcko (1996), *The Properties of Known Drugs. 1. Molecular Frameworks*,
  J. Med. Chem. 39(15)
- `research.md` §7 — evaluation conventions
- [`training/data/scaffold_split.py`](../../training/data/scaffold_split.py) — the
  implementation, with a self-test asserting benzene and toluene co-locate
- [ADR-06](0006-tdc-over-moleculenet.md) — the dataset this splits
- [ADR-04](0004-inchikey-identity-and-cache-key.md) — deduplication must happen
  before splitting, or a duplicate straddles the boundary and leaks regardless
