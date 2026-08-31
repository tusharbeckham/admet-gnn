# ADR-04: Use the InChIKey as molecular identity, and key the cache on `(inchikey, model_version)`

- **Status**: Accepted
- **Date**: 2026-08-15
- **Deciders**: tusharbeckham
- **Related**: FR-05, TR-05, NFR-01, NFR-02, NFR-07, NFR-08

## Context

Two questions need one answer.

**"Have I seen this molecule before?"** A batch of 10,000 vendor-catalogue
compounds routinely contains the same structure several times, written differently
each time. Deduplicating requires deciding when two inputs are the same molecule.

**"Can I reuse this prediction?"** Inference is the expensive step. A cache turns a
repeated molecule from ~5 ms into a hash lookup, which is most of what makes a
10,000-row screen finish inside NFR-02's 90-second budget.

The obvious key is the SMILES string, and it is wrong. These are all aspirin:

```text
CC(=O)Oc1ccccc1C(=O)O
O=C(C)Oc1ccccc1C(=O)O
CC(=O)OC1=CC=CC=C1C(O)=O
```

Keying on the raw string means three cache entries, three database rows, and a
"unique compounds" count that is a function of how the vendor formatted their CSV.

Canonical SMILES fixes the aliasing but introduces a worse problem: **it is
canonical only with respect to one implementation at one version**. RDKit's
canonicalisation is not the same as Open Babel's, and RDKit's own output has
changed between releases (aromaticity perception and stereo handling in
particular). A cache keyed on canonical SMILES silently partitions the moment
RDKit is upgraded, and — far worse — a *database* keyed on it acquires duplicate
rows for the same compound with no error anywhere.

And a cache key has a second dimension nobody notices until it bites: a cached
prediction belongs to the **model that produced it**. Retrain, redeploy, and every
cached entry is now a stale answer being served as a current one.

## Decision

**Identity is the standard InChIKey.** RDKit generates it once, at ingestion, from
the parsed molecule. It is the primary lookup key on `molecules`, typed
`CHAR(27)` with a unique index.

**The prediction cache is keyed on the pair `(inchikey, model_version)`**, never on
`inchikey` alone.

Both the raw input SMILES and the canonical SMILES are stored alongside — the raw
one because a user needs to see what they typed, the canonical one because it is
what gets rendered and compared. Neither is an identity key.

## Consequences

### Positive

- **Aliasing is solved by construction.** All three aspirin strings above produce
  `BSYNRYMUTXBXSQ-UHFFFAOYSA-N`. The "unique compounds" count in a batch report is
  a chemical fact rather than a formatting artefact.
- **`CHAR(27)` is a free constraint.** An InChIKey is exactly 27 characters
  (14 skeleton, 10 stereo/isotope, 1 protonation, with two hyphens). Fixed-width
  means a malformed value fails at insert rather than being stored and confusing
  someone in three weeks.
- **A model upgrade invalidates the cache by construction.** This is the part worth
  the ADR. With `model_version` in the key, deploying a new model means every
  lookup misses — automatically, with no flush step to remember, no TTL to tune,
  and no window during which old and new answers are mixed in one batch report.
  The alternative (key on `inchikey`, flush on deploy) fails silently exactly once
  and produces a report with two models' numbers in it.
- **The InChIKey is portable.** It is an IUPAC standard, so a compound identified
  here can be looked up in PubChem or ChEMBL directly. That is what makes
  cross-referencing an external dataset possible at all.
- **Stereochemistry is respected.** The second block encodes it, so enantiomers get
  different keys — which matters, because thalidomide is the canonical example of
  two enantiomers with different ADMET behaviour.

### Negative

- **Generating an InChIKey is not free.** It runs the InChI algorithm, which is
  measurably slower than canonical SMILES generation — on the order of tens of
  microseconds per molecule. At 10,000 molecules that is a fraction of a second,
  so it is dominated entirely by inference; the cost is real and irrelevant.
- **It is a hash, so it is one-way.** You cannot reconstruct a structure from a
  key. This is why the canonical SMILES is stored too, rather than being
  regenerable.
- **Tautomers get different keys.** The standard InChI does not perform tautomer
  normalisation, so keto and enol forms of the same compound are distinct
  identities. Chemically arguable, and it is a **known limitation to state in the
  report** rather than a bug to fix — tautomer standardisation is its own research
  problem and choosing a canonical tautomer would introduce a different set of
  wrong answers.
- **RDKit is required to compute it.** Which means it happens in the training
  environment (Python) at ingestion, and the Rust side must be able to produce the
  same key for a molecule submitted at request time. That is a parity surface, and
  it is smaller than the featuriser's but not zero.
- **Salts and mixtures need a policy before the key is computed.** `CC(=O)O.[Na+]`
  and `CC(=O)O` are different InChIKeys but the same active compound. The pipeline
  strips salts and keeps the largest organic fragment *before* generating the key,
  and that transformation must be recorded, because it means the stored identity is
  not always the identity of the submitted string.

### Neutral

- `molecules.id` is a UUIDv7 rather than the InChIKey itself. A 27-character
  natural key would work as a primary key, but a 16-byte time-ordered UUID makes
  a narrower foreign key in `predictions`/`prediction_values`, and UUIDv7's
  ordering keeps index inserts sequential. The InChIKey carries the unique index;
  it just is not the PK.
- Cache capacity (50,000 entries, 16 shards) is a tuning number, not part of this
  decision. Set in [`config/default.toml`](../../config/default.toml).

## Alternatives considered

### Raw SMILES as the key

Rejected: three spellings of aspirin, three rows. Every downstream count becomes a
function of input formatting.

### Canonical SMILES as the key

The tempting middle option, and the one most projects take. Rejected because
"canonical" is scoped to an implementation and a version. An RDKit upgrade
re-partitions the cache — annoying — and inserts duplicate rows into `molecules`
for compounds already present — a data-integrity failure with no error message.
The InChIKey is a published standard with a versioned algorithm, which is a
materially stronger guarantee.

### Molecular formula plus molecular weight

Cheap. Rejected: isomers collide, which is the opposite of what an identity key is
for. Ibuprofen and its structural isomers would share a row.

### Cache keyed on `inchikey`, flushed manually on deploy

Rejected because it depends on remembering. The failure is not "the cache is
stale"; the failure is one batch report containing predictions from two different
models, with nothing in the output distinguishing them. Putting `model_version` in
the key makes that state unreachable.

### Content hash of the feature tensor

Would key on exactly what the model consumes, which is appealing. Rejected because
it makes the key depend on the featuriser version as well, so a feature-schema bump
invalidates the *molecule* rows and not just the predictions — and because the key
would then be meaningless outside this system, losing the PubChem/ChEMBL
cross-reference that the InChIKey gives for free.

## References

- IUPAC InChI Trust: <https://www.inchi-trust.org/>
- `crates/admet-db/src/model.rs` — the `Molecule` and `Prediction` shapes
- `method.md` §8 — where in the nine inference steps the key is computed
- [ADR-05](0005-scaffold-split-not-random.md) — the other decision that depends on
  deduplication being correct, since a duplicate across the train/test boundary is
  leakage
