# `desktop/`

Tauri 2 wrapper: the same SvelteKit UI, the same Rust core, no server and no
network. **Empty until Increment 5.**

## Why this exists

Pharmaceutical structure-activity data is the asset. Plenty of teams will not put
an unpublished structure into a web form on someone else's machine at any price,
which makes a hosted-only product unusable for exactly the users who need it most
(G6 and NFR-09: local, offline, zero per-request cost). The desktop build answers
that by never opening a socket, and it is the second half of G7 — one codebase,
two shipped forms — under UC-08 and FR-24.

It is also the payoff for ADR-02. `admet-core` has no I/O, `admet-infer` has no
HTTP, so the Tauri backend links them directly and calls the identical chemistry
and the identical `.onnx` through `invoke` instead of `fetch`. If the domain crate
had a `sqlx::PgPool` in it, this build would be a rewrite. Because it does not, it
is a wrapper.

```
┌─ web build (Increment 3) ─┐      ┌─ desktop build (Increment 5) ─┐
│ SvelteKit → HTTP → axum   │      │ SvelteKit → invoke → Tauri    │
│              ↓            │      │              ↓                │
│      admet-core + infer   │      │      admet-core + infer       │  ← identical
└───────────────────────────┘      └───────────────────────────────┘
```

## Planned scaffold

```bash
cd desktop
pnpm create tauri-app@latest .     # frontend: none (reuses ../web build output)
```

The `web` project keeps one `+layout.ts` flag selecting the transport, so
`api.ts` calls either `fetch('/predict')` or `invoke('predict')` and no component
knows which.

## Windows deltas (ADR-07)

Native Windows rather than WSL2, so:

- **MSVC toolchain required**, not GNU. Needs the *Desktop development with C++*
  workload from Visual Studio Build Tools — the Tauri build fails at link time
  without it, and the error names a missing `link.exe` rather than the workload.
- **WebView2** is the renderer. Present on Windows 11 already; the installer must
  still declare the bootstrapper for older targets.
- **Bundle target is `.msi`/`.exe`**, not `.deb`/`.AppImage`. Cross-compiling
  Tauri bundles is not worth attempting — build each platform on that platform.
- **No SIGTERM.** Irrelevant here (no daemon), but it is why
  `admet-api`'s shutdown path is `#[cfg(unix)]`-gated.

## What ships inside the bundle

`model.onnx` is embedded as a Tauri resource, not downloaded on first run — an
offline tool that needs the network once is not an offline tool. That puts the
artefact size directly in the installer size, which is the real argument for
keeping the model small.
