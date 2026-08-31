# Data Sources, Licences and the Resource Directory

> Build Manual chapter 3. Everything you will need to look up, in one place.

---

## 1. The dataset — Therapeutics Data Commons

Data comes from the **TDC ADMET Benchmark Group**: a curated, peer-reviewed
collection of twenty-two ADMET datasets with standardised train/valid/test
splits. Using it means your numbers are comparable to published work, which is a
genuinely strong point in a viva.

```bash
just data-download
```

That runs [`training/data/download_tdc.py`](../training/data/download_tdc.py) in
the separate `.venv-tdc` environment. **Read the banner in that file before
touching it** — the two-environment split is load-bearing, not fussiness.

TDC prescribes **five split seeds** for the train/validation split. Report mean
and standard deviation across all five. A single run is not a result, and
examiners notice.

### Record the PyTDC version

TDC has revised individual datasets between releases. A benchmark number is
meaningless without knowing which revision produced it. `download_tdc.py` writes
`pytdc_version` into `training/data/raw/tdc/_manifest.json`, and that value must
be copied into the model card. The script also flags row-count **drift** against
the published sizes below — if you see `DRIFT`, note it in your weekly log rather
than ignoring it.

---

## 2. The twelve endpoints you will ship

TDC offers twenty-two. Twelve is the right scope for fifteen weeks: broad enough
to be a real platform, small enough to finish. These twelve span all five ADMET
categories, which is what lets you claim genuine coverage.

| ID | TDC dataset | Category | Task | n | What it actually tells a chemist |
|---|---|---|---|---|---|
| E01 | `Caco2_Wang` | Absorption | regression | 906 | Gut-wall permeability. Low value means the drug will not be absorbed orally |
| E02 | `HIA_Hou` | Absorption | binary | 578 | Human intestinal absorption. A hard gate for any oral drug |
| E03 | `Pgp_Broccatelli` | Absorption | binary | 1,212 | P-glycoprotein substrate. If yes, the body actively pumps the drug back out |
| E04 | `Bioavailability_Ma` | Absorption | binary | 640 | Fraction reaching circulation intact |
| E05 | `BBB_Martins` | Distribution | binary | 1,975 | Blood–brain barrier penetration. Essential for CNS drugs, undesirable for most others |
| E06 | `PPBR_AZ` | Distribution | regression | 1,797 | Plasma protein binding. Highly bound drugs have little free active fraction |
| E07 | `VDss_Lombardo` | Distribution | regression | 1,130 | Volume of distribution — how widely it spreads into tissue |
| E08 | `CYP3A4_Veith` | Metabolism | binary | 12,328 | Inhibition of the enzyme metabolising ~50% of all drugs. Predicts interactions |
| E09 | `CYP2D6_Veith` | Metabolism | binary | 13,130 | Second major metabolic enzyme; highly variable between individuals |
| E10 | `Half_Life_Obach` | Excretion | regression | 667 | How long it survives in the body. Drives dosing frequency |
| E11 | `Clearance_Hepatocyte_AZ` | Excretion | regression | 1,213 | Rate the liver removes it |
| E12 | `hERG` | Toxicity | binary | 648 | Cardiac ion-channel blockade. The classic late-stage killer — the single most valuable early flag |

**The list is frozen.** Adding a thirteenth endpoint requires a written ADR
(risk **R5**). The canonical machine-readable copy is the `ENDPOINTS` dict in
`download_tdc.py`; this table is the human-readable twin. If they ever disagree,
the code wins.

### Why this selection defends well

Five categories, eight classification and four regression tasks, sizes from 578
to 13,130. That range is **deliberate**: it forces you to handle small-data
overfitting *and* larger-batch training, and it gives the results chapter
something honest to discuss.

When the half-life model underperforms — and it will, 667 samples for a
continuous target is genuinely hard — you have a real finding to analyse rather
than a failure to hide. Two endpoints are expected to be difficult:

- **`Pgp_Broccatelli`** — P-gp substrate behaviour is genuinely noisy
- **`Half_Life_Obach`** — 667 rows, high experimental variance

Explaining *why* those are hard demonstrates more understanding than a uniformly
excellent table would.

---

## 3. Licences and citation obligations

| Asset | Licence | Your obligation |
|---|---|---|
| TDC datasets | mostly CC-BY 4.0 | Cite TDC **and** the original dataset paper for each endpoint. The TDC page for each dataset lists its source citation |
| RDKit | BSD-3-Clause | Include the licence text if you redistribute. Cite in references |
| PyTorch | BSD-3-Clause | Attribution only |
| ONNX Runtime | MIT | Attribution only |
| Rust crates | MIT / Apache-2.0 | Generate a bundled notice with `cargo about` |
| SmilesDrawer | MIT | Attribution only |
| Your code | see `LICENSE` | Currently `LicenseRef-Proprietary`. **Note the repo is public — an all-rights-reserved licence restricts *reuse*, not *visibility*.** If you want the code to be reusable, switch to MIT |

### Generating the third-party notice

```bash
cargo install cargo-about && cargo about init
```

```bash
cargo about generate about.hbs > docs/THIRD_PARTY_LICENSES.html
```

```bash
pnpm dlx license-checker --production --summary
```

**Licence compliance is a real deliverable.** Most student projects ignore it
entirely. A generated attribution page shows you understand that shipping
software carries legal obligations, not just technical ones — and it is one
command.

---

## 4. Complete resource directory

| What you need | Where | Use it for |
|---|---|---|
| TDC benchmark overview | `tdcommons.ai/benchmark/admet_group` | Dataset descriptions, metrics, leaderboard to compare against |
| TDC source | `github.com/mims-harvard/TDC` | API reference, issue tracker |
| RDKit documentation | `rdkit.org/docs` | Featurisation, descriptors, scaffold extraction |
| **RDKit cookbook** | `rdkit.org/docs/Cookbook.html` | Copy-paste recipes for almost every chemistry task you will hit |
| ADMET-AI paper | *Bioinformatics* 40(7), btae416 | The state-of-the-art baseline to position against |
| Chemprop docs | `chemprop.readthedocs.io` | Reference D-MPNN implementation. **Read it, do not depend on it** |
| **ONNX operator list** | `onnx.ai/onnx/operators` | Check an operator exists *before* you rely on it. Critical — see ADR-03 |
| `ort` crate docs | `ort.pyke.io` | Rust ONNX Runtime bindings |
| Axum docs | `docs.rs/axum` | Extractors, routing, middleware |
| Axum ecosystem | `github.com/tokio-rs/axum` → `ECOSYSTEM.md` | Curated list of compatible crates |
| sqlx docs | `docs.rs/sqlx` | Compile-time-checked queries, migrations |
| SvelteKit docs | `svelte.dev/docs/kit` | Routing, load functions, form actions |
| **Svelte 5 runes** | `svelte.dev/docs/svelte/what-are-runes` | The new reactivity model. Read this *before* writing components |
| Tauri v2 guide | `tauri.app/start` | Desktop packaging, IPC, sidecar processes |
| UnoCSS | `unocss.dev` | Atomic CSS engine and presets |
| Observable Plot | `observablehq.com/plot` | Chart grammar and examples |
| SmilesDrawer | `smilesdrawer.readthedocs.io` | Client-side 2D structure rendering |
| RDKit.js | `github.com/rdkit/rdkit-js` | WASM chemistry in the browser, if SmilesDrawer is not enough |
| IEEE 830 SRS template | search "IEEE 830 template" | The structure `docs/01-srs.md` follows |
| StarUML docs | `docs.staruml.io` | Diagram creation, export settings |
| Typst documentation | `typst.app/docs` | Synopsis and generated-report syntax |
| k6 documentation | `k6.io/docs` | Load-test scripting for the performance chapter |

---

## 5. What is on disk right now

| Path | Contents | Status |
|---|---|---|
| `data/raw/` | `BBBP.csv`, `delaney-processed.csv`, `tox21.csv[.gz]` | **Superseded** MoleculeNet prototype. Gitignored. See [`training/legacy_moleculenet/`](../training/legacy_moleculenet/README.md) |
| `data/processed/` | `bbbp/`, `esol/`, `tox21/` × train/val/test | Superseded, kept as a reference for what a cleaned split looks like |
| `training/data/raw/tdc/` | *(empty until `just data-download`)* | **Current.** The twelve endpoints land here |
| `results/data_profile.json` | *(empty until `just data-profile`)* | The exploratory summary; feeds the report's data chapter |

Nothing under either `data/` tree is committed — `.gitignore` covers both paths
deliberately, because covering only one is how 22,000 lines of CSV once got
committed by accident. The **only** committed data artefacts are the small
golden-vector fixtures under `fixtures/`, which CI needs in order to verify
feature parity without a download.

---

## 6. Before you model anything

Manual ch. 18.2 step 1, and it is not optional:

```bash
just data-profile
```

That produces sample counts, class balance for binary tasks, target
distributions for regression tasks, heavy-atom range, and the fraction of
molecules above the 128-atom cap. It also tells you which endpoints are
imbalanced enough to need `pos_weight` and which are long-tailed enough to need
Huber loss rather than MSE.

**Surprises here are cheap. The same surprises in week nine are not.**
