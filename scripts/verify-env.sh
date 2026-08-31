#!/usr/bin/env bash
# =============================================================
#  ADMETriage -- environment verification
#  Build Manual, Listing 2.8 (adapted for native Windows + Git Bash)
# =============================================================
#
#  Run this before week 2, and again any time something behaves
#  strangely for no apparent reason. Most projects that fail in week
#  ten actually failed in week one, quietly, because something was
#  installed wrong and nobody noticed.
#
#      bash scripts/verify-env.sh
#
#  Exit code is 0 only when every REQUIRED check passes. Optional
#  checks report but never fail the run -- they are tools you do not
#  need until a later increment.
#
#  EVIDENCE FOR YOUR REPORT (manual ch. 2.10)
#  Save the successful output as a screenshot. It becomes the
#  "Development environment" table in the Project Journey Report and
#  is concrete proof of a controlled, reproducible setup.
#
#  WHY THIS IS NOT THE MANUAL'S SCRIPT VERBATIM
#  The manual assumes Linux/WSL2 with a system `python3` and a native
#  `psql`. This project runs native Windows (ADR-07), so:
#    - Python resolves to .venv/Scripts/python.exe, never system python.
#      System python here is 3.14; the project needs 3.12, and checking
#      the wrong interpreter is worse than not checking at all.
#    - Postgres is checked through `docker exec` so a native psql
#      client is optional.
#    - Every tool is classed required/optional, so a fresh clone gets
#      an actionable list instead of a wall of red.
# =============================================================

set -u

# --- resolve the repository root, however this script was invoked ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT" || exit 1

# --- the project interpreter, not whatever `python` means today ---
VENV_PY="$REPO_ROOT/.venv/Scripts/python.exe"          # Windows
[ -x "$VENV_PY" ] || VENV_PY="$REPO_ROOT/.venv/bin/python"   # POSIX fallback

GREEN=$'\033[32m'; RED=$'\033[31m'; YELLOW=$'\033[33m'
DIM=$'\033[2m';    BOLD=$'\033[1m'; OFF=$'\033[0m'

ok=0; fail=0; skip=0
declare -a MISSING_REQUIRED=()
declare -a MISSING_OPTIONAL=()

# check <label> <command> <required|optional> <install-hint>
check () {
  local label="$1" cmd="$2" tier="$3" hint="${4:-}"
  printf '  %-16s' "$label"
  local out
  if out=$(eval "$cmd" 2>&1); then
    printf '%sOK%s    %s%s%s\n' "$GREEN" "$OFF" "$DIM" \
      "$(printf '%s' "$out" | head -1 | cut -c1-52)" "$OFF"
    ok=$((ok + 1))
  elif [ "$tier" = "required" ]; then
    printf '%sFAIL%s  %s\n' "$RED" "$OFF" "$hint"
    fail=$((fail + 1))
    MISSING_REQUIRED+=("$label")
  else
    printf '%sskip%s  %s%s%s\n' "$YELLOW" "$OFF" "$DIM" "$hint" "$OFF"
    skip=$((skip + 1))
    MISSING_OPTIONAL+=("$label")
  fi
}

section () { printf '\n%s%s%s\n' "$BOLD" "$1" "$OFF"; }

printf '%sADMETriage environment check%s   %s%s%s\n' \
  "$BOLD" "$OFF" "$DIM" "$REPO_ROOT" "$OFF"

# -------------------------------------------------------------
section 'Rust toolchain      (Increment 2 onward -- the serving half)'
# -------------------------------------------------------------
check "rustc"      "rustc --version"           required "install: docs/00-machine-setup.md #rust"
check "cargo"      "cargo --version"           required "install: docs/00-machine-setup.md #rust"
check "rustfmt"    "cargo fmt --version"       required "rustup component add rustfmt"
check "clippy"     "cargo clippy --version"    required "rustup component add clippy"
check "nextest"    "cargo nextest --version"   required "cargo install cargo-nextest --locked"
check "just"       "just --version"            required "cargo install just"
check "sqlx-cli"   "sqlx --version"            optional "needed at Increment 2 (migrations)"
check "llvm-cov"   "cargo llvm-cov --version"  optional "needed for NFR-04 coverage evidence"
check "audit"      "cargo audit --version"     optional "needed for CI supply-chain job"

# -------------------------------------------------------------
section 'Python training env (Increment 1 -- the model half)'
# -------------------------------------------------------------
check "uv"         "uv --version"              required "install: docs/00-machine-setup.md #uv"
check ".venv"      "test -x '$VENV_PY' && '$VENV_PY' --version" required \
                   "uv venv --python 3.12 && uv pip install -r requirements.txt"
check "torch"      "'$VENV_PY' -c 'import torch; print(torch.__version__)'"     required "uv pip install -r requirements.txt"
check "rdkit"      "'$VENV_PY' -c 'import rdkit; print(rdkit.__version__)'"     required "uv pip install -r requirements.txt"
check "onnx"       "'$VENV_PY' -c 'import onnx; print(onnx.__version__)'"       required "uv pip install -r requirements.txt"
check "onnxruntime" "'$VENV_PY' -c 'import onnxruntime as o; print(o.__version__)'" required "uv pip install -r requirements.txt"
check "numpy"      "'$VENV_PY' -c 'import numpy; print(numpy.__version__)'"     required "uv pip install -r requirements.txt"
check "pandas"     "'$VENV_PY' -c 'import pandas; print(pandas.__version__)'"   required "uv pip install -r requirements.txt"
check "sklearn"    "'$VENV_PY' -c 'import sklearn; print(sklearn.__version__)'" required "uv pip install -r requirements.txt"
check "ruff"       "'$VENV_PY' -m ruff --version"                               optional "uv pip install -r requirements.txt"
check "pytest"     "'$VENV_PY' -m pytest --version"                             optional "uv pip install -r requirements.txt"

# PyTDC lives in its OWN environment on purpose -- it pins
# rdkit<2024.3.1 and would drag the project's chemistry core back two
# years. See requirements-data.txt for the full reasoning.
check "PyTDC(.venv-tdc)" \
  "test -x '$REPO_ROOT/.venv-tdc/Scripts/python.exe' && '$REPO_ROOT/.venv-tdc/Scripts/python.exe' -c 'import tdc; print(tdc.__version__)'" \
  optional "separate env -- see requirements-data.txt"

# -------------------------------------------------------------
section 'Services            (Increment 2 onward)'
# -------------------------------------------------------------
check "docker"     "docker --version"          required "install: docs/00-machine-setup.md #docker"
check "postgres"   "docker exec admet-pg pg_isready -U admet" optional "just db-up"

# -------------------------------------------------------------
section 'Front end           (Increment 3 onward)'
# -------------------------------------------------------------
check "node"       "node --version"            required "install: docs/00-machine-setup.md #node"
check "pnpm"       "pnpm --version"            optional "corepack enable && corepack prepare pnpm@9 --activate"

# -------------------------------------------------------------
section 'Documents & tooling'
# -------------------------------------------------------------
check "git"        "git --version"             required "install: git-scm.com"
check "gh"         "gh --version"              optional "cli.github.com -- used for repo/PR automation"
check "typst"      "typst --version"           optional "cargo install --locked typst-cli (synopsis + PDF reports)"

# -------------------------------------------------------------
#  Project-specific invariants. A tool being installed is not the
#  same as the repository being in a usable state.
# -------------------------------------------------------------
section 'Repository invariants'
check "spike model" "test -f fixtures/spike_tiny_gin.onnx" required \
                    "python training/scripts/spike_onnx_export.py"
check "parity fixt" "test -f fixtures/parity/manifest.json" optional \
                    "python training/scripts/dump_parity_fixture.py"
check "py 3.12"     "'$VENV_PY' -c \"import sys; sys.exit(0 if sys.version_info[:2]==(3,12) else 1)\"" required \
                    ".venv must be Python 3.12 (PyTDC and torch wheels expect it)"

# -------------------------------------------------------------
printf '\n%s%s%s\n' "$BOLD" "----------------------------------------------------------" "$OFF"
printf '  passed %s%d%s    failed %s%d%s    optional-missing %s%d%s\n' \
  "$GREEN" "$ok" "$OFF" "$RED" "$fail" "$OFF" "$YELLOW" "$skip" "$OFF"

if [ "${#MISSING_OPTIONAL[@]}" -gt 0 ]; then
  printf '\n  %snot yet needed:%s %s\n' "$DIM" "$OFF" "${MISSING_OPTIONAL[*]}"
fi

if [ "$fail" -eq 0 ]; then
  printf '\n  %sEnvironment is ready.%s Screenshot this for your report.\n\n' "$GREEN" "$OFF"
  exit 0
fi

printf '\n  %sBlocking:%s %s\n' "$RED" "$OFF" "${MISSING_REQUIRED[*]}"
printf '  Fix these before starting the increment that needs them.\n'
printf '  Install commands: %sdocs/00-machine-setup.md%s\n\n' "$BOLD" "$OFF"
exit 1
