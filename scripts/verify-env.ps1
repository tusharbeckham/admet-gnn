# =============================================================
#  ADMETriage -- environment verification (PowerShell twin)
#  Build Manual, Listing 2.8
# =============================================================
#
#      pwsh -File scripts/verify-env.ps1
#
#  Behaviourally identical to scripts/verify-env.sh: same checks, same
#  required/optional tiers, same exit code. Kept as a twin because the
#  bash version needs Git Bash on PATH, and the first thing a fresh
#  Windows machine does NOT have is a reliable POSIX shell. If the two
#  ever disagree about whether the environment is ready, that is a
#  defect in this file.
# =============================================================

$ErrorActionPreference = 'Continue'

$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

# The project interpreter, not whatever `python` resolves to. System
# python on this machine is 3.14; the project needs 3.12.
$VenvPy = Join-Path $RepoRoot '.venv\Scripts\python.exe'
$TdcPy  = Join-Path $RepoRoot '.venv-tdc\Scripts\python.exe'

$script:ok = 0
$script:fail = 0
$script:skip = 0
$script:missingRequired = @()
$script:missingOptional = @()

function Test-Tool {
    param(
        [string]$Label,
        [scriptblock]$Command,
        [ValidateSet('required', 'optional')][string]$Tier = 'required',
        [string]$Hint = ''
    )
    Write-Host ('  {0,-16}' -f $Label) -NoNewline
    $out = $null
    try   { $out = & $Command 2>&1 | Select-Object -First 1 }
    catch { $out = $null }

    if ($LASTEXITCODE -eq 0 -and $out) {
        $detail = [string]$out
        if ($detail.Length -gt 52) { $detail = $detail.Substring(0, 52) }
        Write-Host 'OK   ' -ForegroundColor Green -NoNewline
        Write-Host " $detail" -ForegroundColor DarkGray
        $script:ok++
    }
    elseif ($Tier -eq 'required') {
        Write-Host 'FAIL ' -ForegroundColor Red -NoNewline
        Write-Host " $Hint"
        $script:fail++
        $script:missingRequired += $Label
    }
    else {
        Write-Host 'skip ' -ForegroundColor Yellow -NoNewline
        Write-Host " $Hint" -ForegroundColor DarkGray
        $script:skip++
        $script:missingOptional += $Label
    }
}

function Test-Path-Exists {
    param([string]$Label, [string]$Path, [string]$Tier = 'required', [string]$Hint = '')
    Write-Host ('  {0,-16}' -f $Label) -NoNewline
    if (Test-Path $Path) {
        Write-Host 'OK   ' -ForegroundColor Green -NoNewline
        Write-Host " $(Split-Path -Leaf $Path)" -ForegroundColor DarkGray
        $script:ok++
    }
    elseif ($Tier -eq 'required') {
        Write-Host 'FAIL ' -ForegroundColor Red -NoNewline; Write-Host " $Hint"
        $script:fail++; $script:missingRequired += $Label
    }
    else {
        Write-Host 'skip ' -ForegroundColor Yellow -NoNewline
        Write-Host " $Hint" -ForegroundColor DarkGray
        $script:skip++; $script:missingOptional += $Label
    }
}

function Write-Section { param([string]$Title) Write-Host ''; Write-Host $Title -ForegroundColor White }

Write-Host 'ADMETriage environment check' -ForegroundColor White -NoNewline
Write-Host "   $RepoRoot" -ForegroundColor DarkGray

Write-Section 'Rust toolchain      (Increment 2 onward -- the serving half)'
Test-Tool 'rustc'      { rustc --version }          required 'install: docs/00-machine-setup.md #rust'
Test-Tool 'cargo'      { cargo --version }          required 'install: docs/00-machine-setup.md #rust'
Test-Tool 'rustfmt'    { cargo fmt --version }      required 'rustup component add rustfmt'
Test-Tool 'clippy'     { cargo clippy --version }   required 'rustup component add clippy'
Test-Tool 'nextest'    { cargo nextest --version }  required 'cargo install cargo-nextest --locked'
Test-Tool 'just'       { just --version }           required 'cargo install just'
Test-Tool 'sqlx-cli'   { sqlx --version }           optional 'needed at Increment 2 (migrations)'
Test-Tool 'llvm-cov'   { cargo llvm-cov --version } optional 'needed for NFR-04 coverage evidence'
Test-Tool 'audit'      { cargo audit --version }    optional 'needed for CI supply-chain job'

Write-Section 'Python training env (Increment 1 -- the model half)'
Test-Tool 'uv'          { uv --version }                                            required 'install: docs/00-machine-setup.md #uv'
Test-Path-Exists '.venv' $VenvPy                                                    required 'uv venv --python 3.12; uv pip install -r requirements.txt'
Test-Tool 'torch'       { & $VenvPy -c 'import torch; print(torch.__version__)' }    required 'uv pip install -r requirements.txt'
Test-Tool 'rdkit'       { & $VenvPy -c 'import rdkit; print(rdkit.__version__)' }    required 'uv pip install -r requirements.txt'
Test-Tool 'onnx'        { & $VenvPy -c 'import onnx; print(onnx.__version__)' }      required 'uv pip install -r requirements.txt'
Test-Tool 'onnxruntime' { & $VenvPy -c 'import onnxruntime as o; print(o.__version__)' } required 'uv pip install -r requirements.txt'
Test-Tool 'numpy'       { & $VenvPy -c 'import numpy; print(numpy.__version__)' }    required 'uv pip install -r requirements.txt'
Test-Tool 'pandas'      { & $VenvPy -c 'import pandas; print(pandas.__version__)' }  required 'uv pip install -r requirements.txt'
Test-Tool 'sklearn'     { & $VenvPy -c 'import sklearn; print(sklearn.__version__)' } required 'uv pip install -r requirements.txt'
Test-Tool 'ruff'        { & $VenvPy -m ruff --version }                              optional 'uv pip install -r requirements.txt'
Test-Tool 'pytest'      { & $VenvPy -m pytest --version }                            optional 'uv pip install -r requirements.txt'

# PyTDC lives in its own environment on purpose (it pins rdkit<2024.3.1).
Test-Tool 'PyTDC(.venv-tdc)' { & $TdcPy -c 'import tdc; print(tdc.__version__)' } optional 'separate env -- see requirements-data.txt'

Write-Section 'Services            (Increment 2 onward)'
Test-Tool 'docker'   { docker --version }                       required 'install: docs/00-machine-setup.md #docker'
Test-Tool 'postgres' { docker exec admet-pg pg_isready -U admet } optional 'just db-up'

Write-Section 'Front end           (Increment 3 onward)'
Test-Tool 'node' { node --version } required 'install: docs/00-machine-setup.md #node'
Test-Tool 'pnpm' { pnpm --version } optional 'corepack enable; corepack prepare pnpm@9 --activate'

Write-Section 'Documents & tooling'
Test-Tool 'git'   { git --version }   required 'install: git-scm.com'
Test-Tool 'gh'    { gh --version }    optional 'cli.github.com -- used for repo/PR automation'
Test-Tool 'typst' { typst --version } optional 'cargo install --locked typst-cli (synopsis + PDF reports)'

Write-Section 'Repository invariants'
Test-Path-Exists 'spike model' (Join-Path $RepoRoot 'fixtures\spike_tiny_gin.onnx') required 'python training/scripts/spike_onnx_export.py'
Test-Path-Exists 'parity fixt' (Join-Path $RepoRoot 'fixtures\parity\manifest.json') optional 'python training/scripts/dump_parity_fixture.py'
Test-Tool 'py 3.12' { & $VenvPy -c 'import sys; sys.exit(0 if sys.version_info[:2]==(3,12) else 1)' } required '.venv must be Python 3.12'

Write-Host ''
Write-Host '----------------------------------------------------------' -ForegroundColor White
Write-Host '  passed ' -NoNewline; Write-Host $script:ok -ForegroundColor Green -NoNewline
Write-Host '    failed ' -NoNewline; Write-Host $script:fail -ForegroundColor Red -NoNewline
Write-Host '    optional-missing ' -NoNewline; Write-Host $script:skip -ForegroundColor Yellow

if ($script:missingOptional.Count -gt 0) {
    Write-Host ''
    Write-Host '  not yet needed: ' -ForegroundColor DarkGray -NoNewline
    Write-Host ($script:missingOptional -join ' ')
}

if ($script:fail -eq 0) {
    Write-Host ''
    Write-Host '  Environment is ready.' -ForegroundColor Green -NoNewline
    Write-Host ' Screenshot this for your report.'
    Write-Host ''
    exit 0
}

Write-Host ''
Write-Host '  Blocking: ' -ForegroundColor Red -NoNewline
Write-Host ($script:missingRequired -join ' ')
Write-Host '  Fix these before starting the increment that needs them.'
Write-Host '  Install commands: docs/00-machine-setup.md'
Write-Host ''
exit 1
