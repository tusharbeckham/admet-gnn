# Troubleshooting

| | |
|---|---|
| **System** | ADMETriage |
| **Document** | Failures this stack actually produces on Windows, with the exact symptom text |
| **Status** | Seeded from known Windows/Rust/ONNX/sqlx failure modes. **Append every new one.** |
| **Traces** | [ADR-07](../adr/0007-native-windows-and-training-layout.md) · [`00-machine-setup.md`](../00-machine-setup.md) |

Organised by the **error text you will see**, not by subsystem, because that is what
you have when you need this page. `Ctrl-F` the message.

Native Windows was chosen deliberately over WSL2
([ADR-07](../adr/0007-native-windows-and-training-layout.md)) and the price of that
choice is most of this document. Paying it in advance, written down, is the point.

**When you hit something not listed here, add it** — with the exact message, the cause,
and the fix. A troubleshooting page written from memory at the end of the project
contains the four problems that were memorable, not the twenty that cost time.

## Index

| # | Symptom |
|---|---|
| 1 | `bash: cargo: command not found` right after installing rustup |
| 2 | `error: linker 'link.exe' not found` |
| 3 | `The specified module could not be found. (os error 126)` / `onnxruntime.dll` |
| 4 | `error: set DATABASE_URL to use query macros online` |
| 5 | Parity test fails with nonsense floats after a fresh clone |
| 6 | `Filename too long` on clone or build |
| 7 | `cargo fmt --check` fails on files you never touched |
| 8 | `just db-shell` hangs or prints `the input device is not a TTY` |
| 9 | `python` opens the Microsoft Store |
| 10 | `ImportError: PyTDC is not importable` — or an RDKit version that is not the one you installed |
| 11 | `torch` install pulls gigabytes of CUDA |
| 12 | `Unsupported model IR version` / `NOT_IMPLEMENTED : Could not find an implementation` |
| 13 | `error during connect: ... docker_engine: The system cannot find the file specified` |
| 14 | `Bind for 0.0.0.0:5433 failed: port is already allocated` |
| 15 | `pnpm: command not found` |
| 16 | CI fails a lint that passes locally |
| 17 | Tauri build fails on `webview2` or `WindowsSdk` |
| 18 | `cargo build` takes minutes for a one-line change |
| 19 | Pre-commit hook rejects a commit you need to make now |
| 20 | `just` recipe fails with `unbound variable` |

---

## 1. `bash: cargo: command not found` right after installing rustup

**Cause.** `rustup-init.exe` appends `%USERPROFILE%\.cargo\bin` to the user PATH, but an
already-open Git Bash session inherited the old environment. Nothing is broken.

**Fix.** Close and reopen the terminal. Or, without reopening:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

**Check.** `rustc --version` and `cargo --version` both print, and `just verify` turns
those rows green.

## 2. `error: linker 'link.exe' not found`

**Cause.** The `x86_64-pc-windows-msvc` toolchain — the correct one on Windows — links
with the Microsoft linker, which ships with the Visual Studio C++ build tools, not with
Rust.

**Fix.** Install **Visual Studio Build Tools** and select the *Desktop development with
C++* workload (the MSVC toolset and the Windows SDK are the parts that matter).

```bash
winget install --id Microsoft.VisualStudio.2022.BuildTools
```

Then reopen the terminal so the SDK environment is picked up.

**Do not** switch to the `gnu` toolchain to avoid this. It works until `ort` needs to
load a DLL built for MSVC, and then it fails in a much less obvious place.

## 3. `The specified module could not be found. (os error 126)` mentioning `onnxruntime`

**Symptom.** `cargo build` is clean; the failure is at run time, on `Engine::load`, in
`admet-infer`.

**Cause.** `ort` binds to the ONNX Runtime shared library. Either the download-managed
copy never arrived (offline machine, blocked proxy) or a system copy is on PATH at the
wrong version. Error 126 is Windows for "a dependency of the DLL is missing", so it is
frequently the **Visual C++ redistributable**, not `onnxruntime.dll` itself.

**Fix, in order of likelihood.**

```bash
winget install --id Microsoft.VCRedist.2015+.x64
```

If that is already present, point `ort` at an explicit library instead of letting it
resolve one:

```bash
export ORT_DYLIB_PATH="/c/onnxruntime/lib/onnxruntime.dll"
```

**Then verify the version matches the pin** in `Cargo.toml` (`ort = "=2.0.0-rc.13"`).
A minor mismatch loads and then produces wrong output shapes, which is worse than
failing — this is risk R6, and the exact pin exists because of it.

## 4. `error: set DATABASE_URL to use query macros online, or run cargo sqlx prepare`

**Cause.** `sqlx` checks queries against a real schema at compile time (TR-07). Without
a reachable database and no cached metadata, it cannot.

**Fix — while developing:**

```bash
just db-up
```

```bash
export DATABASE_URL="postgres://admet:changeme@localhost:5433/admet"
```

**Fix — for CI and for building offline:** commit the query metadata.

```bash
cargo sqlx prepare --workspace
```

```bash
export SQLX_OFFLINE=true
```

The `.sqlx/` directory it writes **must be committed**, and must be regenerated in the
same commit as any query change. A stale `.sqlx/` compiles a query that no longer
matches the schema, which is the one failure mode compile-time checking was supposed to
remove.

## 5. Parity test fails with nonsense floats after a fresh clone

**Symptom.** `onnx_parity` fails, and the printed values are not slightly off — they are
absurd (`1e+38`, `NaN`, wildly wrong magnitudes). Nothing in the code changed.

**Cause.** The `.f32` fixtures in `fixtures/parity/` are raw little-endian float32.
Windows checked them out with CRLF translation, so every `0x0A` byte inside a float
became `0x0D 0x0A` and the file is now the wrong length with everything after the first
newline byte shifted.

**This is the single most expensive bug on this platform**, because it looks like a
featuriser defect and sends you reading chemistry code.

**Fix.**

```bash
git check-attr binary -- fixtures/parity/aspirin.f32
```

That must print `binary: set`. If it does not, [`.gitattributes`](../../.gitattributes)
is missing the rule. With the rule in place, re-checkout the files:

```bash
git rm --cached -r fixtures/parity && git checkout -- fixtures/parity
```

**Diagnostic that identifies it in ten seconds:** a float32 blob's size must be
divisible by 4.

```bash
find fixtures/parity -name '*.f32' -exec sh -c 'echo "$(wc -c <"$1") $1"' _ {} \;
```

Any size not divisible by 4 is a mangled file, not a maths problem.

## 6. `Filename too long` on clone or build

**Cause.** The Win32 API's 260-character path limit, hit by nested `target/` and
`node_modules/` paths.

**Fix.** Once per machine, as administrator:

```bash
git config --system core.longpaths true
```

Also enable long paths in Windows itself (`Computer Configuration → Administrative
Templates → System → Filesystem → Enable Win32 long paths`), because git's flag only
covers git.

**Avoid.** Keep the repository shallow in the tree — `C:\projects\Phore` rather than
`C:\Users\<name>\Documents\University\Year 3\Dissertation\Code\…`.

## 7. `cargo fmt --check` fails on files you never touched

**Cause.** `core.autocrlf=true` rewrote line endings on checkout, so every line differs
from what `rustfmt` expects to emit.

**Fix.** This repository sets line endings through
[`.gitattributes`](../../.gitattributes), which is the correct mechanism because it
travels with the repo. Turn the global override off:

```bash
git config --global core.autocrlf input
```

Then normalise the working tree:

```bash
git rm --cached -r . && git reset --hard
```

**Check.** `git diff --stat` is empty and `just fmt-check` passes.

## 8. `just db-shell` hangs, or prints `the input device is not a TTY`

**Cause.** Git Bash is not a Windows console, and `docker exec -it` wants one.

**Fix.**

```bash
winpty docker exec -it admet-postgres psql -U admet -d admet
```

The same applies to any interactive container command. Non-interactive `docker exec`
(as used by `just db-up`'s readiness loop) is unaffected.

## 9. `python` opens the Microsoft Store

**Cause.** Windows ships a stub `python.exe` on PATH that redirects to the Store when no
real Python is registered ahead of it.

**Fix.** Never rely on bare `python`. Every recipe in the [`justfile`](../../justfile)
names the interpreter explicitly, and so should you:

```bash
.venv/Scripts/python.exe -c "import rdkit, torch; print(rdkit.__version__, torch.__version__)"
```

That explicitness is also what prevents the other half of this class of bug: a shell
that has forgotten it had a venv activated and is silently using the system Python.

## 10. `PyTDC is not importable`, or an RDKit version that is not the one you installed

**Cause.** Two environments exist on purpose. PyTDC pins `rdkit<2024.3.1`; this project
runs 2026.3.5. Installing both into one venv silently downgrades RDKit, and then the
featuriser is computing chemistry with a different library than the one the parity
fixture was built against — which is risk R3 arriving through the back door.

**Fix.** Use the right interpreter for the right job. Data download only:

```bash
.venv-tdc/Scripts/python.exe training/data/download_tdc.py
```

Everything else, including all chemistry:

```bash
.venv/Scripts/python.exe training/data/profile.py
```

**Check which RDKit each env has** — they must differ:

```bash
for p in .venv .venv-tdc; do echo -n "$p: "; $p/Scripts/python.exe -c "import rdkit; print(rdkit.__version__)" 2>&1 | tail -1; done
```

**The hard rule this protects:** `download_tdc.py` performs **zero chemistry**. It
writes raw CSV. If you find yourself wanting to canonicalise inside it, that is the
moment the two RDKit versions start disagreeing about your dataset.

## 11. `torch` install pulls gigabytes of CUDA

**Cause.** The default PyPI `torch` wheel bundles CUDA. This project is CPU-only —
inference is Rust + ONNX Runtime on CPU, and training a model this size does not need a
GPU.

**Fix.** Install from the CPU index, as `requirements.txt` specifies:

```bash
uv pip install --python .venv/Scripts/python.exe torch --index-url https://download.pytorch.org/whl/cpu
```

**Check.**

```bash
.venv/Scripts/python.exe -c "import torch; print(torch.__version__, torch.cuda.is_available())"
```

`+cpu` in the version and `False` is correct here, not a problem.

## 12. `Unsupported model IR version`, or `NOT_IMPLEMENTED : Could not find an implementation for the node`

**Symptom.** The model exports fine from PyTorch and fails when ONNX Runtime loads it —
possibly only in Rust, having worked in Python.

**Cause.** An opset mismatch. The graph targets an operator set the runtime does not
implement, or uses an operator absent at the opset you exported to. TR-01 fixes the
opset precisely because this failure appears in the *other* language.

**Fix.** Confirm what the artefact actually claims, rather than what you think it does:

```bash
.venv/Scripts/python.exe -c "import onnx; m = onnx.load('models/model.onnx'); print('ir', m.ir_version, 'opset', [(o.domain or 'ai.onnx', o.version) for o in m.opset_import])"
```

Compare against TR-01 in [`requirements.md`](../../requirements.md) and against
`OPSET` in `training/scripts/spike_onnx_export.py`. **All three must agree.** They do
not today — the spike and TR-01 say 17, commit `fad0660` says 18 — which is open
question Q-1 in [`01-srs.md`](../01-srs.md) §5 and is exactly the kind of drift this
check catches.

Then re-run the spike, which reproduces the whole round trip in seconds and needs no
data:

```bash
just spike
```

If a specific node is unimplemented, it is almost always a sparse-scatter operator that
crept back in. The dense-adjacency design
([ADR-03](../adr/0003-dense-adjacency-over-sparse-scatter.md)) exists to keep the graph
inside the exportable operator set; `torch.bmm` is the whole point.

## 13. `error during connect: ... docker_engine: The system cannot find the file specified`

**Cause.** Docker Desktop is not running. On Windows the CLI talks to a named pipe that
only exists while the engine is up.

**Fix.** Start Docker Desktop, wait for the whale icon to stop animating, then:

```bash
docker info --format '{{.ServerVersion}}'
```

**Note.** Docker Desktop uses a WSL2 backend internally. That is not a contradiction of
[ADR-07](../adr/0007-native-windows-and-training-layout.md), which is about where *this
project's* toolchain lives — Rust, Python and Node run natively; only the Postgres
container is virtualised, and it is reached over TCP on 5433 like any other host.

## 14. `Bind for 0.0.0.0:5433 failed: port is already allocated`

**Cause.** A previous `admet-postgres` container is still running, or something else
holds 5433.

**Fix.**

```bash
just db-down && just db-up
```

If that is not it, find the holder:

```bash
docker ps -a --filter publish=5433
```

```bash
netstat -ano | grep 5433
```

**Why 5433 and not 5432:** a system Postgres install takes 5432, and two servers on one
port produce connection failures that look like authentication failures.

## 15. `pnpm: command not found`

**Cause.** pnpm ships with Node via corepack but is not enabled by default.

**Fix.**

```bash
corepack enable pnpm
```

If corepack itself is missing, Node is too old or was installed without it; reinstall
Node 22 LTS per [`00-machine-setup.md`](../00-machine-setup.md).

## 16. CI fails a lint that passes locally

**Cause.** CI runs Linux; development is Windows ([ADR-07](../adr/0007-native-windows-and-training-layout.md)
records this as a known gap). The three things that differ, in order of how often they
bite:

1. **Filesystem case sensitivity.** `use crate::Features;` compiles on Windows when the
   file is `features.rs`. On Linux it does not.
2. **Path separators in string literals.** A hard-coded `"docs\\evidence"` works on one
   and not the other. Build paths with `Path::join`.
3. **A newer clippy in CI.** New lints arrive with each toolchain release, and
   `-D warnings` promotes them to errors.

**Fix — reproduce it before pushing.** The whole point of `just ci-local` is that it is
the same gate:

```bash
just ci-local
```

For a lint that is genuinely CI-only, match the toolchain:

```bash
rustup update stable && cargo clippy --workspace --all-targets -- -D warnings
```

## 17. Tauri build fails on `webview2` or a missing Windows SDK

**Cause.** Tauri on Windows needs the **WebView2 runtime** (present on Windows 11, but
absent in some stripped images) and the same MSVC toolchain as §2.

**Fix.**

```bash
winget install --id Microsoft.EdgeWebView2Runtime
```

Then confirm the Rust side is happy before involving the frontend:

```bash
cargo build -p admet-api --release
```

**Why this is in Increment 5 and tested on Windows from week one:** risk R8. A Tauri
build that has never run on the development platform is discovered to be broken in the
last week, and there is no time to find out whether the cause is Tauri, WebView2 or the
bundle configuration.

## 18. `cargo build` takes minutes for a one-line change

**Cause.** Windows Defender scanning every artefact `rustc` writes into `target/`. This
is the largest single build-time cost on this platform and it is invisible.

**Fix.** Exclude the build directory from real-time scanning. In an **administrator**
PowerShell:

```powershell
Add-MpPreference -ExclusionPath "C:\projects\Phore\target"
```

Consider `%USERPROFILE%\.cargo` too. Do not exclude the whole source tree — the
trade-off is worth making for generated artefacts, not for files you download.

**Measure it rather than believing it.** Time a touch-and-rebuild before and after:

```bash
touch crates/admet-core/src/lib.rs && time cargo build -p admet-core
```

## 19. The pre-commit hook rejects a commit you need to make now

**Cause.** [`.githooks/pre-commit`](../../.githooks/pre-commit) runs `fmt --check`,
clippy, `admet-core` tests and a staged-diff secret scan. One of them failed.

**Fix — the right one first.** Read what it said; the hook prints the failing gate. Most
often:

```bash
just fmt && git add -u
```

**Escape hatch**, for a work-in-progress commit on a branch:

```bash
git commit --no-verify
```

Use it knowingly and rarely. The hook exists because `admet-core`'s suite runs in about
a second ([ADR-02](../adr/0002-hexagonal-crate-split.md)), which is the only reason a
pre-commit test gate is tolerable at all — and a gate that gets routinely bypassed is a
gate that has stopped working.

**One case where `--no-verify` is wrong regardless:** the secret scan. If it fired, do
not bypass it. Remove the credential, and if it was ever committed, treat it as
compromised and rotate it — history rewriting does not un-publish a pushed secret.

## 20. A `just` recipe fails with `unbound variable`

**Cause.** The [`justfile`](../../justfile) sets `set shell := ["bash", "-uc"]`. The
`-u` makes an unset variable an error rather than an empty string, so a typo in a
variable name fails loudly instead of expanding to nothing.

That is deliberate: without it, `cargo test -p ` silently tests the whole workspace and
you believe you ran a targeted test.

**Fix.** Supply the variable the recipe expects:

```bash
just predict "CC(=O)Oc1ccccc1C(=O)O"
```

```bash
just bench-cli N=10000
```

`just --list` shows each recipe's parameters, and `just --show <recipe>` prints its
body.

---

## When it is not on this list

Work in this order — it is roughly cheapest-first:

1. **`just verify`.** A surprising share of failures are a missing tool reported three
   layers down as something else.
2. **Reproduce it in the smallest unit.** `just spike` for anything ONNX (seconds, no
   data). `just test-core` for anything chemistry (about a second, no database, no
   model). If both are green, the problem is in an adapter, not the domain.
3. **Read the actual error, all of it.** Rust's are long and the useful line is often
   the third `note:`.
4. **Check the artefact rather than the code** — file sizes, opset versions, RDKit
   versions, `git check-attr`. Half of this page is artefacts, not logic.
5. **Then write the new entry here**, with the exact message. And if the failure reached
   a passing test suite, it is also a `DEF-nn` in
   [`03-test-plan.md`](../03-test-plan.md) §10.2, and it owes an answer to "which test
   would have caught this?"

## Appendix A — Revision history

| Version | Date | Change |
|---|---|---|
| 0.1 | 2026-08-27 | Seeded with twenty entries covering the toolchain, ONNX/`ort`, sqlx, git line endings and Windows-specific paths, ports and scanning. |