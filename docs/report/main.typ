// =============================================================================
//  ADMETriage — final report
// =============================================================================
//
//  Build:  just report      ->  docs/report/ADMETriage.pdf
//
//  ---------------------------------------------------------------------------
//   The one rule this file exists to enforce
//  ---------------------------------------------------------------------------
//   NO NUMBER IN THIS REPORT IS TYPED BY HAND.
//
//   Every figure is read from a JSON artefact produced by the pipeline, using
//   Typst's `json()`. That is not cleverness for its own sake: a report with
//   hand-copied numbers goes stale the first time the pipeline is re-run, and
//   the failure is silent — the document still compiles, still looks finished,
//   and is quietly wrong. Reading the artefact means a stale number becomes a
//   *missing* number, which is loud.
//
//   Consequence to accept: this file will not compile if the artefacts it reads
//   are absent. That is the intended behaviour. `just results` regenerates them.
//
//   Where a figure genuinely does not exist yet, it is written as an explicit
//   #todo() marker that renders in red and is impossible to miss on a page. A
//   TARGET is never presented as a MEASUREMENT.

#set document(title: "ADMETriage", author: "Tushar")
#set page(paper: "a4", margin: (x: 2.4cm, y: 2.6cm), numbering: "1")
#set text(font: ("Times New Roman", "DejaVu Serif", "Liberation Serif"), size: 11pt)
#set par(justify: true, leading: 0.72em)
#set heading(numbering: "1.1")
#show heading: it => block(above: 1.4em, below: 0.8em, it)
#show raw: set text(font: ("Consolas", "DejaVu Sans Mono"), size: 9.5pt)

// A marker for anything not yet produced. Red, bracketed, unmissable.
#let todo(body) = text(fill: red, weight: "bold")[[TODO: #body]]

// A figure that does not exist yet. Distinct from #todo so the two can be
// counted separately when judging whether the report is ready.
#let pending(what, when) = block(
  fill: rgb("#fff4f4"),
  stroke: (left: 3pt + red),
  inset: 8pt,
  width: 100%,
)[#text(fill: red, weight: "bold")[PENDING] — #what #h(1fr) #emph[due: #when]]

// Thousands separators. `37289` is a value; `37,289` is a figure in a report, and
// the difference is whether a reader has to count digits.
#let num(n) = {
  let s = str(calc.round(n))
  let out = ()
  let i = s.len()
  while i > 3 {
    out.push(s.slice(i - 3, i))
    i -= 3
  }
  out.push(s.slice(0, i))
  out.rev().join(",")
}

// =============================================================================
//  Data loaded from the pipeline. Adding a `json()` call here is how a number
//  gets into the report.
// =============================================================================

#let profile = json("../evidence/increment-1/data_profile.json")
#let schema = json("../../models/feature_schema.json")

// =============================================================================

#align(center)[
  #v(3cm)
  #text(size: 26pt, weight: "bold")[ADMETriage]
  #v(0.4cm)
  #text(size: 14pt)[An explainable graph neural network platform \ for early-stage ADMET screening]
  #v(2cm)
  #text(size: 11pt)[
    Twelve ADMET endpoints · multi-task dense GIN · Rust serving path
  ]
  #v(1fr)
  #text(size: 10pt, style: "italic")[
    Status: scaffold. No model has been trained. \
    Every performance figure in this document is a target and is labelled as one.
  ]
  #v(2cm)
]

#pagebreak()
#outline(depth: 2, indent: auto)
#pagebreak()

= Introduction

== Problem

#todo[write from `research.md` §1 — the attrition argument, with the citation]

== Aim and objectives

#todo[import the numbered objectives from `docs/01-srs.md` §1.2 verbatim, so the
report and the SRS cannot disagree]

== Scope

The system predicts #profile.endpoints.len() ADMET endpoints from a SMILES
string, returns a per-endpoint prediction with an applicability-domain judgement,
and ranks a batch by composite triage score. It is a *screening* aid: it orders
candidates for a chemist's attention and does not replace an assay.

= Literature and background

#todo[from `research.md` — GNN families, why message passing suits molecules,
what the published ADMET baselines are]

== Why a graph, not a fingerprint

#todo[from `method.md` §2]

= Method

== Molecular representation

Each atom is encoded as a #(schema.n_features)-dimensional feature vector, laid out
in #schema.blocks.len() blocks. The layout is generated from a single source of
truth in `admet-core` and exported to `models/feature_schema.json`, which the
Python featuriser reads; both ends are tested against it.

#figure(
  table(
    columns: (auto, auto, auto, 1fr),
    align: (left, right, right, left),
    stroke: 0.4pt + gray,
    table.header[*Block*][*Offset*][*Width*][*Encodes*],
    ..schema.blocks.map(b => (
      raw(b.name),
      str(b.offset),
      str(b.width),
      if b.name == "element" [one-hot over #schema.element_order.join(", ")]
      else if b.name == "hybridisation" [one-hot over #schema.hybridisation_order.join(", ")]
      else if b.name == "formal_charge" [buckets #schema.charge_buckets.map(str).join(", ")]
      else if b.name == "degree" [buckets #schema.degree_buckets.map(str).join(", ")]
      else if b.name == "num_hs" [buckets #schema.hydrogen_buckets.map(str).join(", ")]
      else [boolean flag],
    )).flatten(),
  ),
  caption: [The #(schema.n_features)-feature atom contract, schema version
    #schema.schema_version. Generated from `models/feature_schema.json`; the
    numbers in this table cannot drift from the code that produces them.],
)

Out-of-range values clamp rather than error: #schema.clamping

== Network architecture

The atom axis is fixed at #schema.max_heavy_atoms, which is what makes the graph
exportable to ONNX at all — see ADR-03. Section 4.3 quantifies what that cap
costs on real data.

#pending[Fig 9 — dense GIN layer diagram (`docs/diagrams/09-gin.excalidraw`)][Increment 1]

#todo[the message-passing equations from `method.md` §3, and the multi-task head]

== Splitting strategy

#todo[from ADR-05 — Bemis–Murcko, and why a random split inflates every score]

= Data

== Sources

All twelve endpoints come from the Therapeutics Data Commons ADMET benchmark
group; ADR-06 records the choice of TDC over MoleculeNet, and
`docs/06-data-sources.md` carries the licence obligations per asset.

== Profile

The corpus is #num(profile.total_rows) labelled rows across
#profile.endpoints.len() endpoints, with zero unparseable SMILES.

#figure(
  table(
    columns: 8,
    align: (left, left, right, right, right, right, right, right),
    stroke: 0.4pt + gray,
    table.header[*Endpoint*][*Task*][*Rows*][*Bad*][*Heavy μ*][*Max*][*\>cap*][*Dup*],
    ..profile.endpoints.pairs().map(((key, e)) => (
      raw(e.tdc_name),
      if e.task == "regression" [reg] else [bin],
      num(e.rows),
      str(e.unparseable),
      str(calc.round(e.heavy_mean, digits: 1)),
      str(e.heavy_max),
      str(e.over_cap),
      str(e.duplicate_smiles),
    )).flatten(),
  ),
  caption: [Dataset profile, generated from `data_profile.json`. "\>cap" counts
    molecules exceeding the #(profile.max_heavy_atoms)-atom limit; they are
    rejected, never truncated, because a truncated molecule is a different
    molecule.],
)

=== What the cap costs

#let over = profile.endpoints.values().map(e => e.over_cap).sum()
Exactly #over molecules of #num(profile.total_rows) exceed the
#(profile.max_heavy_atoms)-heavy-atom cap — #calc.round(100.0 * over / profile.total_rows, digits: 3)%
of the corpus. ADR-03 traded a fixed atom axis for ONNX exportability, and this
is the price of that trade, measured rather than assumed.

=== Class balance and target skew

#let imbalanced = profile.endpoints.pairs().filter(((k, e)) => e.task != "regression" and (e.at("positive_frac", default: 0.5) < 0.3 or e.at("positive_frac", default: 0.5) > 0.7))
#let skewed = profile.endpoints.pairs().filter(((k, e)) => e.at("long_tailed", default: false))

#skewed.len() of the regression endpoints are long-tailed, the most extreme being
a skew of
#calc.round(profile.endpoints.values().map(e => e.at("target_skew", default: 0.0)).fold(0.0, (a, b) => if calc.abs(b) > calc.abs(a) { b } else { a }), digits: 1).
That rules out plain MSE: on a target this skewed, MSE optimises for the
outliers. Huber loss is used instead.

#todo[state the per-endpoint `pos_weight` values, reading them from the profile
once `training/report.py` emits them]

= Design and implementation

#todo[from `docs/02-design.md` — the hexagonal split, and why `admet-core` has no
I/O (ADR-02)]

#pending[Figs 1–2 — C4 context and container diagrams][Increment 2]

= Database

#pending[Fig 7 — ER diagram][Increment 2]

#todo[schema, the `(inchikey, model_version)` cache key from ADR-04]

= Testing

#todo[import the pyramid table and final counts from `docs/03-test-plan.md`; the
defect log is §10.2 there and should be summarised, not duplicated]

= Results

#pending[Every number in this chapter. No model has been trained.][Increment 1]

#todo[per-endpoint AUROC / MAE, 5 seeds, mean ± sd — and the comparison against
the published TDC leaderboard]

= Performance

#pending[NFR-01 and NFR-02 measurements on a quiet 2-vCPU host][Increment 3]

Component benchmarks exist for the pure functions and are recorded in
`docs/evidence/increment-0/benchmarks.md`. They are deliberately *not* quoted
here as latency results: they exclude featurisation, ONNX inference, HTTP and
cache, and presenting a 2.56 ms sort as evidence for a 300 ms end-to-end budget
would overstate the case by three orders of magnitude.

= Deployment and operations

#pending[Fig 8 — deployment diagram][Increment 4]

#pending[Fig 16 — CI pipeline and its gates][Increment 5]

= Evaluation and reflection

#todo[what worked, what did not, and what the measured numbers say about the
choices recorded in the ADRs — including any ADR this work proved wrong]

= Conclusion

#todo[write last]

#pagebreak()

= References

#todo[BibTeX or a manual list, per the submission requirements]

#pagebreak()

= Appendix A — Architecture decision records

The seven ADRs are maintained in `docs/adr/` and are the authoritative record.

#todo[include them verbatim here, or cite the repository — check the submission
rules on appendices before deciding]

= Appendix B — Defect log

#todo[reproduce `docs/03-test-plan.md` §10.2. Fifteen defects were recorded
before Increment 1 finished; three were S1, and two were quality gates that had
never once executed.]
