# ADR-02: Split the Rust side into five crates, with zero I/O in `admet-core`

- **Status**: Accepted
- **Date**: 2026-08-13
- **Deciders**: tusharbeckham
- **Related**: TR-02, TR-11, NFR-04, NFR-06, NFR-11, G7, G8, R8

## Context

The Rust side does five distinguishable things: chemistry, inference, storage,
HTTP, and batch/benchmark work from a terminal. A single crate could hold all
five, and for a project this size that is a defensible default — Rust's module
system already gives you namespacing, so extra crates buy nothing automatically.

Three specific forces push the other way.

**Test speed decides whether tests get run.** The chemistry code is where the
subtle bugs live: ring perception, aromaticity, stereo, the 33-feature layout.
That code needs a test suite dense enough to be run on every save. If those tests
live in the same crate as `sqlx` and `axum`, then `cargo test` compiles a
connection pool and an HTTP stack before it can check whether benzene's ring
closure parses, and a suite that takes ninety seconds gets run at coffee breaks
instead of continuously.

**Increment 5 needs the chemistry without the server.** The desktop build (G7) is
the same UI and the same model with no network. If the chemistry code can reach a
`PgPool`, that build is a refactor. If it cannot, it is a wrapper.

**Compile-time coupling is the coupling that hurts.** A `use` of `sqlx` inside a
featuriser is not a design smell you notice in review; it is a dependency edge
that makes the featuriser untestable without a database, and it appears one
convenient import at a time.

## Decision

Five crates, in a Cargo workspace, with dependency arrows pointing inward:

```text
                    admet-cli ──┐
                                ├──> admet-infer ──> (ort)
                    admet-api ──┤
                        │       └──> admet-core   ← NO I/O
                        │                ▲
                        └──> admet-db ───┘
```

| Crate | Owns | Forbidden |
|---|---|---|
| `admet-core` | SMILES parsing, `MolGraph`, canonicalisation, 33-feature vectors, fingerprints, scaffolds, triage scoring | **all I/O**: no `sqlx`, `axum`, `tokio`, `reqwest`, `std::fs` on a hot path |
| `admet-infer` | ONNX session lifecycle, tensor marshalling, batching | HTTP, SQL |
| `admet-db` | Postgres schema, repositories, migrations | chemistry, HTTP |
| `admet-api` | HTTP, routing, middleware, config, RFC 9457 errors | chemistry, SQL strings |
| `admet-cli` | batch CSV scoring, benchmarks | HTTP, SQL, async runtime |

The constraint on `admet-core` is stated positively as: **its only dependencies
are `thiserror`, `serde`, and (in dev) `proptest`/`criterion`.** That list is short
enough to audit in one glance at `crates/admet-core/Cargo.toml`, which is the
point — a rule you can check in two seconds is a rule that survives week nine.

Internal path dependencies are declared once in the root
[`Cargo.toml`](../../Cargo.toml) `[workspace.dependencies]` block, so reading that
block top to bottom is how you verify the arrows still point inward.

## Consequences

### Positive

- **`cargo test -p admet-core` is seconds, not minutes.** That is why the
  [pre-commit hook](../../.githooks/pre-commit) can run it on every commit
  without being bypassed. The hook and this ADR are the same decision seen twice.
- **The desktop build is a wrapper.** Tauri links `admet-core` + `admet-infer` and
  calls them through `invoke`. No server, no port, no database. See
  [desktop/README.md](../../desktop/README.md).
- **`admet-cli` needs no async runtime.** No tokio, no sqlx, no axum in its
  dependency tree — which is what makes `admet-cli bench` an honest benchmark
  rather than a measurement of the stack.
- **Property tests are cheap.** `proptest` over the SMILES parser runs thousands
  of cases per second because there is nothing to set up. Round-trip properties
  (parse → canonicalise → parse gives the same graph) are the highest-value tests
  in the project and they are only affordable here.
- **Compile times stay tolerable.** Changing a handler recompiles `admet-api`
  alone. In a single crate it recompiles the featuriser too.

### Negative

- **More `Cargo.toml` files, more version churn.** Five manifests, and a dependency
  bump touches several. Mitigated by `[workspace.dependencies]`, which makes each
  member's entry `foo.workspace = true` — but it is still five files.
- **Types have to cross boundaries explicitly.** `admet-db` cannot store an
  `admet_core::MolGraph` directly; it stores the fields and reconstructs. That is
  a real conversion layer with real code in it, and a single crate would not need
  it. The upside is that the database schema is then a *deliberate* design rather
  than whatever the domain struct happened to look like.
- **The rule needs enforcing, not just stating.** Nothing in Cargo prevents
  someone adding `sqlx` to `admet-core` at 2 a.m. The defence is the short
  dependency list plus review; `cargo-deny` can make it mechanical, and should
  once there is a reason to.
- **`admet-api` must be a library *and* a binary.** In a binary crate, unused
  `pub` items warn, so a scaffold that defines the response payload before the
  handler that fills it cannot compile under `-D warnings`. The lib+bin split
  fixes that and also lets integration tests drive the router with
  `tower::ServiceExt::oneshot` without binding a socket — but it is an extra file
  and an extra concept.

### Neutral

- `migrations/` lives at the repo root rather than inside `admet-db`, because
  `sqlx-cli`, docker compose and CI all resolve `./migrations` from the invocation
  directory. `admet_db::MIGRATIONS_DIR` names the path once. This is a documented
  deviation from `implementation.md` §8; see [migrations/README.md](../../migrations/README.md).
- `admet_infer::Engine::run` takes `&mut self`, so shared access currently needs a
  `Mutex`. That lock is a known bottleneck, documented where it is created in
  `crates/admet-api/src/state.rs`, and Increment 2 replaces it with a single
  worker task plus an mpsc channel — which is also what implements micro-batching
  (NFR-02, TR-08). The crate split is what makes that swap a change in one file.

## Alternatives considered

### One crate with modules

Simplest, and Rust modules already provide namespacing. Rejected on the two
concrete costs: `cargo test` would compile `axum` and `sqlx` before running a ring
perception test, and the Increment 5 desktop build would have no way to exclude the
server. Both are consequences you feel every day, not architectural purity.

### Two crates: `admet-lib` and `admet-api`

The pragmatic middle. It fixes nothing that matters, because the whole problem is
that chemistry and persistence would still share a crate — so the fast test suite
still pulls in `sqlx`, and the desktop build still links a connection pool.

### Traits and dependency injection instead of crate boundaries

Define `trait MoleculeStore` in the domain and implement it in the adapter. This
is the textbook hexagonal answer, and it was rejected as *premature*: there is
exactly one implementation of each repository, so the trait's only benefit is
enabling in-memory fakes — and a fake Postgres is a worse test than a real
Postgres in a container, because the bugs in this layer are SQL bugs (a wrong
`ON CONFLICT`, a missing `NULLS LAST`) that a fake reproduces perfectly and
therefore hides. The crate boundary already gives the compile-time separation;
the traits would only add indirection. Revisit if a second backend ever appears.

### Separate repositories per component

Independent versioning, independent CI. Rejected outright for a solo fifteen-week
project: cross-repo changes need coordinated PRs, and the first time the feature
schema changes you would need three of them.

## References

- Cockburn, *Hexagonal Architecture* — the dependency-inversion argument this
  follows loosely rather than dogmatically
- `implementation.md` §8 — the layout this refines
- [ADR-01](0001-rust-serving-onnx-boundary.md) — the ONNX seam that makes
  `admet-infer` separable at all
- `crates/admet-core/Cargo.toml` — the short dependency list that is the actual
  enforcement mechanism
