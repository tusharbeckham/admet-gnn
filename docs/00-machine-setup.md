# Machine Setup — Native Windows

> Build Manual chapter 2, translated for this machine. Every command below is
> the *Windows* form; the manual's Linux/WSL2 commands will not work verbatim
> and are not reproduced.
>
> **Run `bash scripts/verify-env.sh` first.** It tells you which of these
> sections you actually need. Do not install things the script says are already
> present.

---

## 0. Hardware reality check

You do **not** need a GPU. This is worth stating plainly, because it is the most
common reason students abandon an ML project before starting.

| Resource | Minimum | Comfortable | Note |
|---|---|---|---|
| CPU | 4 cores | 8 cores | **Rust compilation** is the bottleneck, not training |
| RAM | 8 GB | 16 GB | 8 GB works if you close the browser while compiling |
| Disk | 25 GB free | 50 GB free | Rust `target/` dirs are large; Docker adds ~5 GB |
| GPU | none | none | The whole model trains on CPU in 20–40 minutes |

ADMET benchmark datasets are small by deep-learning standards — 578 to 13,130
molecules per endpoint. A 3-layer GNN over 128-atom graphs is a handful of small
matrix multiplies. **This is a genuine advantage of the domain and worth saying
in your viva.**

---

## Current state of this machine

Audited at scaffold time. Update this table when it changes.

| Tool | Status | Version |
|---|---|---|
| `uv` | installed | 0.12.5 |
| `node` | installed | **v24.14.1** — see §Node below |
| `docker` | installed | 29.6.1 |
| `git` | installed | 2.55.0.windows.3 |
| `gh` | installed | 2.96.0 |
| `.venv` python | installed | 3.12.10 |
| torch / rdkit / onnx / onnxruntime | installed | 2.13.0+cpu / 2026.3.5 / 1.22.0 / 1.29.0 |
| **`rustc` / `cargo`** | **MISSING** | — install below |
| **`just`** | **MISSING** | — install below |
| `pnpm` | missing | needed at Increment 3 |
| `sqlx-cli` | missing | needed at Increment 2 |
| `typst` | missing | needed for the synopsis |
| `psql` | missing | optional — use `docker exec` instead |

---

## 1. Rust {#rust}

**Install via `rustup`, never via a package manager.** Distro and Chocolatey
Rust packages run months behind and you will hit version conflicts with crates.

Download and run the installer:

```bash
curl -o "$TEMP/rustup-init.exe" https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe
```

```bash
"$TEMP/rustup-init.exe" -y --default-toolchain stable --profile default
```

The installer will tell you if the **MSVC build tools** are missing. They are a
hard requirement — `ort`, `sqlx` and `ring` all compile C. If prompted, accept
the automatic Visual Studio Build Tools install, or get it yourself:

```bash
winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

**Close and reopen your terminal**, then verify:

```bash
rustc --version && cargo --version
```

`rust-toolchain.toml` at the repo root pins `channel = "stable"` with `rustfmt`
and `clippy`, so those two arrive automatically the first time you run `cargo`
inside the repo.

> **Why the repo pins a channel, not a version.** The manual pins `1.83.0`.
> That is right for a team shipping to production. For a solo project on a
> deadline it mostly means rustup downloading a second toolchain and then
> quietly rotting for fifteen weeks. The comment in `rust-toolchain.toml`
> records this. Revisit it only if CI and your laptop ever disagree about a
> warning.

### Cargo tools

Each takes a few minutes to compile. `just` and `cargo-nextest` are needed
immediately; the rest can wait for the increment that uses them.

```bash
cargo install just
```

```bash
cargo install cargo-nextest --locked
```

```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres
```

```bash
cargo install cargo-llvm-cov cargo-audit
```

> Note `--features rustls` rather than the manual's default. The default
> `sqlx-cli` build links native TLS, which on Windows means hunting for OpenSSL.
> `rustls` is pure Rust and just works.

---

## 2. Python — already done, do not touch {#python}

`.venv` is Python **3.12.10** with everything in `requirements.txt` installed.
Leave it alone. The one thing to know:

**Always invoke the interpreter by path.** `python` on this machine resolves to
system Python 3.14, which is *not* the project interpreter.

```bash
.venv/Scripts/python.exe training/scripts/spike_onnx_export.py
```

Or activate first, in which case bare `python` is correct for that shell only:

```bash
source .venv/Scripts/activate
```

Every recipe in the `justfile` uses the explicit path, so `just` is always safe.

### The second Python environment

`PyTDC` is **deliberately not** in `.venv`. PyTDC 1.1.15 pins
`rdkit>=2023.9.5,<2024.3.1`; this project runs rdkit 2026.3.5. Installing PyTDC
into the main environment would drag the cheminformatics core back two and a
half years — and splitting chemistry across two RDKit versions is exactly how
feature skew (risk **R3**) gets in, invisibly, until metrics disagree.

So TDC gets a throwaway environment, used once per endpoint to fetch raw CSV:

```bash
uv venv .venv-tdc --python 3.12 && .venv-tdc/Scripts/python.exe -m pip install -r requirements-data.txt
```

```bash
just data-download
```

**Hard rule:** `training/data/download_tdc.py` performs *no chemistry*. It
fetches and writes raw CSV, nothing else. Every RDKit operation — salt
stripping, canonicalisation, InChIKey, scaffolds, featurisation — happens in
`.venv` against the single pinned rdkit.

---

## 3. Node and pnpm {#node}

Node **v24.14.1** is installed. The manual specifies **22 LTS**.

`.nvmrc` and `package.json`'s `packageManager` field pin 22, and CI uses 22.

**Recommendation: install Node 22 and use it for this project.** Not because 24
breaks SvelteKit — it does not — but because "works on my machine" is a defect
category, and a laptop/CI version mismatch is the cheapest possible source of
one. Use `fnm` (fast, Rust, Windows-native):

```bash
winget install Schniz.fnm
```

```bash
fnm install 22 && fnm use 22 && node --version
```

If you decide to stay on 24 instead, that is defensible — but **record it**:
change `.nvmrc` and the CI `node-version` to 24 together, so the two never
disagree. Do not leave them mismatched.

Then enable pnpm through corepack, which ships with Node:

```bash
corepack enable && corepack prepare pnpm@9 --activate && pnpm --version
```

Corepack will refuse to run under the wrong package manager once
`packageManager` is set, which prevents the classic mixed-lockfile mess.

---

## 4. PostgreSQL — via Docker {#postgres}

Docker 29.6.1 is installed. **Do not install Postgres natively.** Use the
disposable container:

```bash
just db-up
```

That is the manual's Option A and it is strongly preferred while developing.
When you inevitably corrupt your schema experimenting with migrations:

```bash
docker rm -f admet-pg && just db-up
```

Four seconds to a clean slate. That safety net encourages the kind of
experimentation that teaches you the most.

You do not need a native `psql`. Reach the container's own client:

```bash
docker exec -it admet-pg psql -U admet -d admet_dev -c "select version();"
```

---

## 5. Typst {#typst}

Compiles the synopsis, and later the PDF reports the application generates —
one tool for both, so you only learn one syntax.

```bash
cargo install --locked typst-cli
```

Or, if you would rather not wait for it to compile:

```bash
winget install --id Typst.Typst
```

---

## 6. Diagram tools {#diagrams}

Sixteen diagrams are graded artefacts. Tool assignment per diagram is in
[docs/diagrams/README.md](diagrams/README.md). Install what you need, when you
need it (weeks 3–4).

| Tool | Where | Install note |
|---|---|---|
| **StarUML** | staruml.io/download | Primary tool. Free to evaluate indefinitely with a periodic nag dialog. UML 2, ERD, DFD, flowcharts |
| **draw.io** | app.diagrams.net or desktop | Free, no account. Best for architecture and deployment diagrams |
| **dbdiagram.io** | dbdiagram.io | Browser. Write DBML text, get an ER diagram. Fastest route to a clean ERD |
| **Umbrello** | apps.kde.org/umbrello | Draw at least the use-case diagram here for departmental compatibility |
| **PlantUML** | plantuml.com | Text-based sequence/class diagrams that live in Git and diff properly |
| **Mermaid** | built into GitHub Markdown | Zero install. Use for the Gantt chart in your README |

```bash
winget install --id draw.io.drawio
```

**Export at 300 DPI or SVG, always.** Screenshots of diagram tools look
terrible in print — blurry text, compression artefacts. Every tool listed
supports proper export. Five-second decision, visibly affects how the report
reads.

---

## 7. Editor {#editor}

VS Code or Zed. **The extensions matter more than the editor.**

| Extension | Why |
|---|---|
| **rust-analyzer** | Non-negotiable. Inline types, instant errors, refactoring. Rust without it is painful |
| Svelte for VS Code | Syntax and type checking inside `.svelte` files |
| Even Better TOML | `Cargo.toml` editing with schema validation |
| Ruff | Python linting and formatting, extremely fast |
| Tinymist / Typst LSP | Live preview while writing the synopsis |
| Error Lens | Diagnostics inline. Tightens the feedback loop noticeably |

```bash
code --install-extension rust-lang.rust-analyzer --install-extension svelte.svelte-vscode --install-extension tamasfe.even-better-toml --install-extension charliermarsh.ruff --install-extension myriad-dreamin.tinymist --install-extension usernamehw.errorlens
```

---

## 8. Install the git hooks {#hooks}

Versioned hooks, so they survive a fresh clone:

```bash
just hooks
```

That sets `core.hooksPath` to `.githooks/`. See
[docs/05-git-conventions.md](05-git-conventions.md) for what the hook enforces
and why it deliberately stays under three seconds.

---

## 9. Verify the whole thing {#verify}

```bash
bash scripts/verify-env.sh
```

```bash
pwsh -File scripts/verify-env.ps1
```

Both must agree. **Screenshot the passing output** — it becomes the
"Development environment" table in the Project Journey Report, and it is
concrete proof of a controlled, reproducible environment. That kind of detail is
what separates a strong submission from an average one.

Save the screenshot to `docs/evidence/` with today's date in the filename.

---

## Windows-specific gotchas

Collected here so you recognise them instead of debugging them. These are the
predictable costs of ADR-07 (native Windows over WSL2).

| Symptom | Cause | Fix |
|---|---|---|
| `error: linker 'link.exe' not found` | MSVC build tools missing | §1, install VS Build Tools |
| `cargo install sqlx-cli` fails on OpenSSL | native-tls default | Use `--features rustls,postgres` |
| `ort` fails to find `onnxruntime.dll` at runtime | DLL not beside the exe | `ort` downloads and stages it; if it fails, set `ORT_DYLIB_PATH` |
| Long-path errors during `cargo build` | Windows 260-char limit | `git config --global core.longpaths true`, and keep the repo near the drive root — `C:\projects\Phore` is already good |
| Line endings churn in diffs | CRLF | Already handled: `.gitattributes` sets `* text=auto eol=lf` |
| `just` recipe fails with `/bin/sh not found` | No POSIX shell | The `justfile` sets `shell := ["bash", "-uc"]`; ensure Git Bash is on PATH |
| Tauri build fails | WebView2 missing | Windows 11 ships it. Verify: `winget list Microsoft.EdgeWebView2Runtime` |
| Docker volume permission errors | Windows/Linux uid mismatch | Use named volumes (`admet_pgdata`), not bind mounts, as `just db-up` does |

---

## What the manual says about this, and why we diverged

Manual §2.1: *"On Windows, use WSL2. Native Windows will cost you days on RDKit
and toolchain issues."*

That advice was correct for years and is now partly stale — RDKit ships working
Windows wheels, and this machine already has rdkit 2026.3.5 installed and
functioning. Reinstalling the whole Python environment inside WSL to obey the
manual would cost a day and gain nothing on the training side.

The real Windows cost is on the *serving* side: MSVC toolchain setup, the
`sqlx-cli` TLS flag, and Tauri prerequisites. All three are one-time and are
documented above.

This deviation is recorded as **[ADR-07](adr/ADR-07-windows-and-training-dir.md)**
so it reads as a decision with known trade-offs, not an oversight. If you hit
something the table above does not cover and it costs more than an afternoon,
switching to WSL2 remains available — the repo is platform-agnostic apart from
these scripts.
