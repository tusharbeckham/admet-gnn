# `models/`

The Python↔Rust boundary. Everything the serving path needs and nothing else.

**Empty until Increment 1.** `admet-api` starts anyway, logs `no model artefact;
predictions unavailable`, passes `/healthz` and reports the gap on `/readyz`. No
prediction is ever served from that state — `/predict` returns a typed 503.

## What lands here

| File | Produced by | Committed? |
|---|---|---|
| `model.onnx` | `just export` | **No** — see below |
| `model.onnx.sha256` | `just export` | Yes |
| `feature_schema.json` | `just schema` | Yes |
| `metrics.json` | `just train` | Yes |
| `calibration.json` | `just train` | Yes |

## Why the `.onnx` itself is gitignored

It is a binary blob of tens of megabytes that changes on every training run. Git
stores each version in full, so a fortnight of experiments leaves a repository
nobody can clone. What is committed instead is the **checksum**, which is what
you actually need: `model_versions.onnx_sha256` in the database records which
artefact produced a given prediction, so a result can be traced to a file even
after the file is long gone.

Distribute the artefact as a GitHub Release asset attached to the version tag.
That gets you a stable URL, and the tag ties the weights to the exact commit that
trained them.

## `feature_schema.json` is the one that prevents wrong answers

The 33-dimensional atom featuriser exists twice — once in Python for training,
once in Rust for serving (risk R3, TR-03). If the two disagree about what column
17 means, nothing crashes. The model gets a plausible vector, returns a plausible
number, and it is wrong. No test catches that unless something forces the two
implementations to share a definition.

This file is that something. Rust is the source of truth:

```bash
just schema      # cargo run -p admet-cli -- schema > models/feature_schema.json
```

The Python featuriser loads it and asserts the block offsets and widths it is
about to write into. `schema_version` is checked against
`admet_core::features::SCHEMA_VERSION` at server start-up, and a mismatch is a
**refusal to start**, not a warning — a warning here produces confidently wrong
numbers, which is worse than an outage because nobody notices.

## `metrics.json`

Per-endpoint held-out scores under **both** split strategies (scaffold and
random), because the gap between them is the honest measure of generalisation and
reporting only the flattering one is the most common way an ADMET paper misleads.
Every figure in the report's results tables is read from this file by `just
results` — never hand-copied.

## `calibration.json`

Isotonic or Platt parameters per classification endpoint. A GIN's raw sigmoid
output is not a probability, and presenting it as "87 % likely to cross the
blood-brain barrier" without calibration is a claim the model never made.
