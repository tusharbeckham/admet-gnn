# `web/`

SvelteKit 2 + TypeScript + Tailwind front end. **Empty until Increment 3.**

The CI `web` job ships commented out until this directory has a `package.json`,
because a red badge from week one teaches you to ignore the badge, which defeats
having one.

## Planned scaffold

```bash
cd web
pnpm create svelte@latest .        # skeleton, TypeScript, no demo app
pnpm add -D tailwindcss @tailwindcss/vite vitest @playwright/test
pnpm add smiles-drawer
```

Node **22 LTS** (`.nvmrc`), pnpm via corepack. The machine has v24 installed; see
[docs/00-machine-setup.md](../docs/00-machine-setup.md) for why pinning 22
matters and how.

## Planned layout

```
web/
  src/routes/
    +page.svelte              single-molecule scoring        UC-01
    batch/+page.svelte        CSV upload, progress, ranking  UC-02, UC-03
    compare/+page.svelte      side-by-side two molecules     UC-05
    login/+page.svelte        session login                  UC-07
  src/lib/
    api.ts                    typed fetch wrapper over the Rust API
    types.ts                  generated from the OpenAPI spec, never hand-written
    components/
      MoleculeCanvas.svelte   smiles-drawer + the atom-attribution overlay
      RadarChart.svelte       twelve endpoints at a glance
      DomainBadge.svelte      in-domain / borderline / out-of-domain
```

## Two decisions worth making before writing any of it

**`types.ts` is generated, not written.** The API's response shape is defined in
Rust (`crates/admet-api/src/routes/predict.rs`). Typing it a second time by hand
means the two drift, and the symptom is `undefined` in a chart six weeks later.
Generate from the OpenAPI document.

**The atom-attribution overlay is the feature, not a decoration.** A number with
no explanation is a number a chemist cannot act on — that is the whole thesis of
the project (G4, and UC-04 end to end: FR-19 computes the per-atom scores, FR-20
paints them). `MoleculeCanvas` colouring the atoms that drove the prediction
is what makes it a triage tool instead of a lookup table. Budget accordingly: it
is the hardest component here, and the only one users will remember.

## What the front end must never do

No chemistry. No SMILES validation beyond a length check, no descriptor
computation, no "helpful" client-side canonicalisation. The moment the browser
computes something the Rust side also computes, there are two answers to the same
question and one of them is wrong. `smiles-drawer` renders; it does not decide.
