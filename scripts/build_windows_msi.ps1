Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

param(
  [switch]$Install
)

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent $ScriptDir

if (-not $IsWindows) {
  throw "This script must be run on Windows."
}

Write-Host "[1/5] Checking WiX Toolset (v3)..."
$candle = Get-Command candle.exe -ErrorAction SilentlyContinue
$light = Get-Command light.exe -ErrorAction SilentlyContinue
if (-not $candle -or -not $light) {
  throw @"
WiX Toolset v3 is required (candle.exe/light.exe not found).
Install WiX v3 (ex: 3.14.x) and ensure candle.exe/light.exe are in PATH.
"@
}

Write-Host "[2/5] Ensuring cargo-wix is installed..."
$hasCargoWix = $false
try {
  & cargo wix -V | Out-Null
  $hasCargoWix = $true
} catch {
  $hasCargoWix = $false
}
if (-not $hasCargoWix) {
  cargo install cargo-wix --locked
}

Write-Host "[3/5] Building release binary..."
Push-Location $RepoRoot
try {
  cargo build --release --manifest-path (Join-Path $RepoRoot "Cargo.toml")
} finally {
  Pop-Location
}

Write-Host "[4/5] Preparing WiX source..."
$wixDir = Join-Path $RepoRoot "wix"
$mainWxs = Join-Path $wixDir "main.wxs"
if (-not (Test-Path $mainWxs)) {
  Push-Location $RepoRoot
  try {
    cargo wix init
  } finally {
    Pop-Location
  }
}

Write-Host "[5/5] Building MSI..."
Push-Location $RepoRoot
try {
  if ($Install) {
    cargo wix --install
  } else {
    cargo wix
  }
} finally {
  Pop-Location
}

$msiDir = Join-Path $RepoRoot "target\wix"
$msi = Get-ChildItem -Path $msiDir -Filter *.msi -File -ErrorAction SilentlyContinue |
  Sort-Object LastWriteTime -Descending |
  Select-Object -First 1

if ($null -eq $msi) {
  throw "MSI was not found in $msiDir"
}

Write-Host ""
Write-Host "MSI generated:"
Write-Host "  $($msi.FullName)"
