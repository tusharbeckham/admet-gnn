# Glossary

| | |
|---|---|
| **System** | ADMETriage |
| **Document** | Chemistry, machine-learning and engineering terms used across this repository |
| **Status** | Complete for the vocabulary in use at the scaffold tag. Append as terms arrive. |
| **Traces** | [`01-srs.md`](../01-srs.md) §1.3 · [`method.md`](../../method.md) |

Two audiences read this project's documents and each finds a different half opaque: a
chemist who does not know what a CSR adjacency is, and a software assessor who does
not know what hERG is. Every term either audience could stall on is defined here once,
rather than parenthetically in six places.

Definitions are short, and where a term carries a consequence for this system, the
consequence is stated. "Heavy atom: an atom that is not hydrogen" is a dictionary
entry; "…and the count of them is what FR-04 rejects above 128" is why it is here.

**Bold** terms inside a definition are defined elsewhere in this file.

---

## 1. Molecular identity and structure

| Term | Meaning |
|---|---|
| **ADMET** | Absorption, Distribution, Metabolism, Excretion, Toxicity — the five properties that decide whether a compound with good target activity can be a drug. Most late-stage failures are ADMET failures, not potency failures, which is why predicting them early has value. |
| **SMILES** | Simplified Molecular-Input Line-Entry System. A molecule as an ASCII string: `CC(=O)Oc1ccccc1C(=O)O` is aspirin. The system's only input (FR-01). Not unique — one molecule has many valid SMILES, which is what **canonicalisation** fixes. |
| **Canonical SMILES** | The one SMILES string a given algorithm produces for a molecule regardless of how it was written. Makes string comparison meaningful. Algorithm-specific: RDKit's canonical form and another toolkit's may differ, so it is a display and re-parsing aid, not an identity key. |
| **InChI** | IUPAC International Chemical Identifier. A long, layered, standardised string. Toolkit-independent, unlike **canonical SMILES**. |
| **InChIKey** | A fixed-length 27-character hash of an **InChI**, e.g. `BSYNRYMUTXBXSQ-UHFFFAOYSA-N`. Always exactly 27 characters with hyphens at positions 15 and 26. This project's identity column ([ADR-04](../adr/0004-inchikey-identity-and-cache-key.md)) and half of the cache key (TR-05). Being a hash, it cannot be inverted — which is why `canonical_smiles` is stored beside it and is not redundant. |
| **Heavy atom** | Any atom that is not hydrogen. Hydrogens are usually implicit in **SMILES** and are not nodes in the graph. The count is the size measure the 128-atom cap applies to (FR-04). |
| **Aromaticity** | A ring's delocalised-electron character, written lower-case in **SMILES** (`c1ccccc1`). Two toolkits can disagree about a borderline ring, and that disagreement is a real source of **featuriser** skew: RDKit derives it from ring perception, a naive lexer from letter case. |
| **SSSR** | Smallest Set of Smallest Rings. The ring-perception result the graph needs before ring features can be computed. Implemented in `admet-core` via union-find. |
| **Salt / counter-ion** | A charged partner shipped with the active molecule (`…C(=O)[O-].[Na+]`). Different salts of one drug should not become different molecules in the database, so a stripping policy must be fixed **before** the **InChIKey** is computed — open question Q-2 in the SRS. |
| **Tautomer** | Two structures differing only in the position of a proton and a double bond, interconverting in solution. Chemically the same substance; different **InChIKey**s unless normalised. Same open question as salts. |
| **Bemis–Murcko scaffold** | A molecule reduced to its ring systems plus the linkers joining them, with all side chains removed. Benzene and toluene share a scaffold. The grouping key for the **scaffold split**. |

## 2. The twelve endpoints

Frozen by [ADR-06](../adr/0006-tdc-over-moleculenet.md); additions require an ADR
(risk R5). `Code` is the stable short key used in the database, the API and the model's
output order. Row counts are the TDC dataset sizes.

| Code | TDC dataset | Category | Task | Rows | What it tells a chemist |
|---|---|---|---|---|---|
| `caco2` | `Caco2_Wang` | absorption | regression | 906 | Permeability across an intestinal-cell monolayer — a proxy for oral absorption. |
| `hia` | `HIA_Hou` | absorption | binary | 578 | Whether the compound is absorbed from the human intestine at all. |
| `pgp` | `Pgp_Broccatelli` | absorption | binary | 1,212 | P-glycoprotein substrate: an efflux pump that pushes the drug back out, cutting exposure and limiting brain penetration. |
| `bioavail` | `Bioavailability_Ma` | absorption | binary | 640 | Fraction reaching circulation intact after an oral dose. |
| `bbb` | `BBB_Martins` | distribution | binary | 1,975 | Blood–brain barrier penetration. Required for a CNS drug, a liability for anything else. **The vertical-slice endpoint** — largest clean binary set, so it is built end to end first. |
| `ppbr` | `PPBR_AZ` | distribution | regression | 1,797 | Plasma protein binding rate. Only the unbound fraction is active, so 99 % binding means a hundredfold weaker effective dose. |
| `vdss` | `VDss_Lombardo` | distribution | regression | 1,130 | Volume of distribution at steady state — how far out of the bloodstream the drug spreads into tissue. |
| `cyp3a4` | `CYP3A4_Veith` | metabolism | binary | 12,328 | Inhibition of the enzyme metabolising roughly half of all marketed drugs. Inhibiting it is the classic drug–drug interaction. |
| `cyp2d6` | `CYP2D6_Veith` | metabolism | binary | 13,130 | Inhibition of a second major metabolising enzyme, notable for genetic variation between patients. |
| `half_life` | `Half_Life_Obach` | excretion | regression | 667 | Time for plasma concentration to halve. Sets the dosing schedule. |
| `clearance` | `Clearance_Hepatocyte_AZ` | excretion | regression | 1,213 | Rate the liver removes the compound. |
| `herg` | `hERG` | toxicity | binary | 648 | Blockade of a cardiac potassium channel — the canonical cardiotoxicity screen and a common reason a programme is killed outright. |

Two features of that table drive design decisions elsewhere. The **row counts span
twenty-fold** (578 to 13,130), which is why a shared multi-task trunk is used rather
than twelve independent models — risk R2. And **`herg` is a veto, not a score**: a
compound can be excellent on eleven endpoints and worthless because of this one, which
is the argument for a geometric mean in FR-17.

## 3. Deterministic descriptors and drug-likeness

Everything in this section is **computed exactly**, never predicted. NFR-10 requires
the UI to make that distinction visible, and NFR-05 requires the values to equal
RDKit's exactly rather than approximately.

| Term | Meaning |
|---|---|
| **Deterministic descriptor** | A property calculable from the structure by a fixed rule — molecular weight, ring count. Same input, same output, forever, with no model involved. The opposite of a **prediction**, and the distinction the UI must carry. |
| **cLogP** | Calculated octanol/water partition coefficient: lipophilicity. Around 1–3 is usually workable; above 5, absorption and toxicity problems cluster. |
| **TPSA** | Topological polar surface area, in Å². Correlates with permeability; above roughly 90 Å², **BBB** penetration becomes unlikely. |
| **Lipinski's Rule of Five** | Four thresholds (MW ≤ 500, cLogP ≤ 5, H-bond donors ≤ 5, acceptors ≤ 10) suggesting oral availability. FR-07 reports each **individually**, not as a violation count, because "2 violations" hides which two and a chemist needs to know. |
| **Veber rules** | Two further oral-availability thresholds: rotatable bonds ≤ 10, TPSA ≤ 140 Å². Also reported per rule. |
| **QED** | Quantitative Estimate of Drug-likeness: one number in `[0,1]` from eight descriptors. Convenient and shallow — useful as a sort key, not as a decision. |
| **PAINS** | Pan-Assay INterference compoundS. Substructures that produce false positives in many assays. Reported as the *name* of each matched pattern, because "1 alert" is unactionable. |
| **Brenk alerts** | A second substructure filter flagging groups undesirable in a lead compound (reactive, unstable, toxicophoric). |
| **SA score** | Synthetic accessibility, 1 (easy) to 10 (hard), from fragment frequencies in known synthesised molecules. A brilliant molecule nobody can make is not a candidate. |

## 4. Similarity and the applicability domain

| Term | Meaning |
|---|---|
| **Morgan fingerprint / ECFP** | A molecule as a fixed-length bit vector: circular atom environments up to a radius are hashed into bit positions. This project uses radius 2, 2,048 bits, held as `Fingerprint([u64; 32])` so similarity is 32 `popcount`s rather than a loop over bits. |
| **Tanimoto coefficient** | Similarity between two fingerprints: shared bits ÷ total distinct bits. `1.0` identical, `0.0` nothing in common. Above ~0.7 chemists usually call two molecules analogues. |
| **Applicability domain** | The region of chemical space where a model's predictions are supportable, defined here as mean **Tanimoto** to the five nearest training molecules. Below 0.45 → `low_confidence`; below 0.30 → `out_of_domain` with the triage score withheld (FR-11, FR-12). |
| **Domain status** | The three-valued verdict `in_domain` / `low_confidence` / `out_of_domain`, using one vocabulary in the Rust enum, the database column and the JSON. Returned with **200**, not an error — an error code invites a retry, whereas a successful response carrying a refusal forces the caller to handle the honest answer. |
| **Scaffold split** | Train/test division by **Bemis–Murcko scaffold** group, so no scaffold appears on both sides. Harder and more honest than a random split, which lets a near-duplicate of a training molecule into the test set and inflates every metric. [ADR-05](../adr/0005-scaffold-split-not-random.md) requires reporting **both** so the gap is visible. |
| **Desirability** | A per-endpoint mapping of a raw prediction into `[0,1]` where 1 is good, so that quantities in different units and directions can be combined. `PredictionValue.desirability` stores it. |
| **Triage score** | The weighted **geometric** mean of the twelve desirabilities (FR-17). Geometric, so one near-zero endpoint sinks the compound; an arithmetic mean lets eleven good values hide one fatal one, and both return a number in `[0,1]` so a range check would not notice the substitution. |

## 5. Machine learning

| Term | Meaning |
|---|---|
| **GNN** | Graph Neural Network. Operates on the molecular graph directly rather than on a fingerprint, so it can learn which substructures matter instead of being told. |
| **GIN** | Graph Isomorphism Network. A GNN variant provably as discriminative as the Weisfeiler–Lehman test — expressive enough for chemistry, simple enough to export. |
| **Message passing** | The GNN step: each atom updates its vector from its neighbours' vectors. Repeated *k* times, an atom sees *k* bonds out. |
| **Dense adjacency** | The neighbour structure as a padded `[B, 128, 128]` matrix, so one round of message passing is `torch.bmm(adj, x)`. Chosen over sparse scatter because `torch_scatter` has **no ONNX equivalent** — [ADR-03](../adr/0003-dense-adjacency-over-sparse-scatter.md), risk R1. Costs memory on small molecules and buys an exportable graph. |
| **Symmetric normalisation** | Scaling the adjacency by `D^(-1/2) A D^(-1/2)` so a highly connected atom does not dominate. Part of the feature contract and therefore a parity surface. |
| **Mask** | The `[B, 128]` boolean marking real atoms in a padded tensor. Without it, padding is 128 phantom atoms silently averaged into the readout. |
| **Multi-task learning** | One trunk, twelve heads. Endpoints with 578 rows borrow representation learned from those with 13,130 — the mitigation for risk R2. |
| **Masked loss** | Loss computed only over endpoints a molecule actually has labels for. Most molecules have one or two of twelve, so an unmasked loss would train against absence. |
| **AUROC** | Area under the ROC curve, for binary endpoints. 0.5 is chance, 1.0 perfect. NFR-03's target is a mean of ≥ 0.80 on the scaffold-held-out split. |
| **AUPRC** | Area under the precision–recall curve. More informative than **AUROC** when positives are rare, which they are for several endpoints. |
| **Spearman ρ** | Rank correlation, the regression-endpoint metric. Rank rather than magnitude, because ranking candidates is what the system is for. |
| **Calibration** | Whether a predicted 0.7 corresponds to 70 % of such cases being positive. An uncalibrated probability shown as a percentage is misleading rather than merely inaccurate — FR-09, open question Q-3. |
| **Integrated gradients** | The attribution method for FR-19: accumulates gradients along a path from a baseline to the input, yielding a per-atom contribution that sums to the prediction difference. |
| **Seed** | The integer fixing every random choice. TDC prescribes five (1–5); results are reported as mean ± sd over all five, because a single run is not a result (NFR-07, TR-12). |

## 6. Export and serving

| Term | Meaning |
|---|---|
| **ONNX** | Open Neural Network Exchange: a portable graph format. The **only** artefact crossing from Python to Rust ([ADR-01](../adr/0001-rust-serving-onnx-boundary.md)), paired with `feature_schema.json`. |
| **Opset** | The ONNX operator-set version a graph targets. TR-01 fixes it, because an operator available at one opset may be absent at another and the failure appears at load time in the *other* language. |
| **ONNX Runtime / ORT** | The inference engine. Used from Rust via the `ort` crate, pinned to an exact version (risk R6). |
| **Dynamic axis** | A tensor dimension allowed to vary at run time. Only the **batch** axis is dynamic here; atoms and features are fixed at 128 and 33 so shape errors surface at export rather than under load. |
| **Micro-batching** | Collecting concurrent single-molecule requests into one batched inference, up to 64. What makes NFR-02 reachable without twelve separate sessions (TR-08). |
| **Featuriser** | The code turning a molecule into `x`, `adj`, `mask`. It exists **twice**, in Python and Rust, which is risk R3 — skew produces plausible *wrong* predictions rather than errors. |
| **Golden fixture** | Committed feature vectors, generated **from Rust** and asserted **by Python** to 1e-6 (TR-03). The direction is the contract: Rust serves traffic, so Rust defines truth. Editing the fixture to make a test pass is the exact failure it exists to prevent. |
| **Parity** | Agreement between the two implementations. Two tolerances: 1e-6 for features (TR-03), 1e-4 end to end (TR-04). |
| **p95 / p99** | The latency below which 95 % / 99 % of requests complete. NFR-01 is p95 < 300 ms, p99 < 600 ms, warm. A mean would hide the tail, and the tail is what a user notices. |
| **Warm vs cold** | Warm: caches populated, process running. Cold: neither. NFR-01 specifies warm, so quoting only the warm figure is technically compliant and practically misleading — hence `TC-P-002`. |

## 7. Software architecture and tooling

| Term | Meaning |
|---|---|
| **Hexagonal architecture** | Domain logic at the centre with no knowledge of I/O; adapters at the edges. Here it is five crates where `admet-core` depends on nothing but `thiserror` ([ADR-02](../adr/0002-hexagonal-crate-split.md)). Three payoffs: a ~1 s domain suite the pre-commit hook can afford, a desktop build that is a wrapper rather than a rewrite, and no chemistry bug able to hide behind a mock. |
| **Crate** | A Rust compilation unit. `admet-core`, `-infer`, `-db`, `-api`, `-cli`. |
| **Workspace** | One `Cargo.toml` governing all crates: shared dependency versions, one `target/`, one `cargo build`. |
| **SoA** | Struct-of-arrays. `MolGraph` holds parallel arrays (one per atom property) rather than an array of atom structs, so featurisation walks contiguous memory. |
| **CSR** | Compressed Sparse Row. Adjacency as an offsets array plus a neighbours array — the compact form inside `admet-core`, converted to **dense adjacency** only at the tensor boundary. |
| **LL(1)** | A parser that decides with one token of lookahead. Chosen for **SMILES** because it gives an exact byte offset for an error, which FR-02 requires: "invalid at byte 7" is actionable, "invalid SMILES" is not. |
| **Union-find** | The disjoint-set structure used for **SSSR** ring perception. |
| **LRU cache** | Least-recently-used eviction. Sharded 16 ways here so concurrent requests do not contend on one lock. |
| **Axum / tower** | The HTTP framework and its middleware model. Layer order is load-bearing: the body-size limit must run before the JSON parser, or a 21 MB body is parsed before being rejected (TR-06, `TC-S-001`). |
| **sqlx** | The database library. Queries are checked against a live schema **at compile time** (TR-07), so a renamed column breaks the build rather than a request. |
| **Argon2id** | The password-hashing function for FR-23. Memory-hard, so GPU cracking is expensive. Stored as a PHC string carrying its own salt and parameters. |
| **RFC 9457** | The problem-details standard for HTTP error bodies (TR-09): `type`, `title`, `status`, `detail`, plus extensions — here a `position` member for the **SMILES** byte offset. |
| **Tauri** | The desktop shell (Increment 5). Wraps the same web UI in a native window with the API compiled in, so no socket is opened — the argument for G6/NFR-09 and the second half of G7. |
| **SvelteKit / runes** | The web framework and Svelte 5's reactivity primitives. |
| **just** | The task runner. A command in prose gets typed slightly differently each time and the fourth variation wastes an afternoon; a recipe is the command, versioned. |
| **uv** | The Python environment and package manager. Two environments here: `.venv` for training, `.venv-tdc` for data download, because PyTDC pins `rdkit<2024.3.1` and this project runs 2026.3.5 (risk R3). |
| **nextest** | The Rust test runner, used for its JUnit XML output. |
| **llvm-cov** | Coverage. NFR-04 sets a floor of 75 % on `admet-core` and `admet-infer` — those two because a defect there is *silent*: a broken route returns the wrong status code and someone notices within a day; a broken feature column returns a number, and nobody notices at all. |
| **criterion / oha** | Micro-benchmarks and HTTP load generation respectively. Both produce numbers into `results/`; neither gates a build, because a latency threshold on shared CI hardware produces flaky failures and teaches people to re-run the job. |
| **proptest** | Property-based testing. Generates arbitrary input to establish NFR-06, that the parser never panics. Hand-written cases test the malformed inputs one *imagines*; the generator finds the others. |

## 8. Project vocabulary

Terms with a specific meaning inside this repository, which is exactly why they are
worth pinning down.

| Term | Meaning |
|---|---|
| **Increment** | One of five delivered slices, each ending in a git tag (TR-10). Ordered by defensibility, so 1–3 alone present as a complete system if time runs out (risk R9). |
| **Target vs measurement** | The central honesty rule. A **target** is a number in `requirements.md` that a script has not yet produced. A **measurement** is a number in `results/`, produced by `just results`, with hardware and commit recorded. Every figure in the report is one or the other, labelled, and a missed target is recorded as a miss rather than edited. |
| **Evidence** | An artefact that makes a claim checkable — a screenshot, an environment capture, an inspection record. Lives in [`docs/evidence/`](../evidence/README.md), dated and committed, because a screenshot on a desktop is a memory of having checked. |
| **`→` in a table cell** | A task assigned to a named increment, not an omission. Used throughout [`04-traceability.md`](../04-traceability.md). |
| **`✅` in a table cell** | Exists in the repository today, stub or complete. |
| **Stub** | A module with real type signatures, doc comments and `#[ignore]`d test placeholders, but no body — deliberately containing no `todo!()` so that `cargo build` and `clippy -D warnings` are green from the first commit. |
| **Defect (`DEF-nn`)** | A recorded fault, numbered when **found** rather than when fixed, in [`03-test-plan.md`](../03-test-plan.md) §10.2. Severity S1 (a wrong number that looks right) outranks S2 (a broken requirement) deliberately: an outage is noticed in minutes, a plausible wrong prediction is acted on. |
| **Vertical slice** | Building one endpoint — `bbb` / `BBB_Martins` — through *every* layer before widening to twelve. Avoids the state where twelve half-trained endpoints, a half-written API and no working page give three plausible causes for every bug and no way to bisect them. |
| **Requirement register** | [`requirements.md`](../../requirements.md), the authoritative source for every `FR`/`TR`/`NFR`/`UC`/`G`/`R` identifier and its wording. Goals and risks are written **without** a hyphen (`G1`, `R7`); everything else with one. |

## Appendix A — Revision history

| Version | Date | Change |
|---|---|---|
| 0.1 | 2026-08-27 | Written at the scaffold tag. Eight sections: identity, the twelve endpoints, descriptors, similarity, machine learning, export/serving, architecture, project vocabulary. |