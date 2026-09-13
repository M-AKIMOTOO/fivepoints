Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent $ScriptDir

$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\FivePointsCalc"
$ExeName = "fivepoints_calc.exe"
$ExePath = Join-Path $InstallDir $ExeName
$CmdPath = Join-Path $InstallDir "fivepoints_calc.cmd"
$IconPath = Join-Path $InstallDir "fivepoints_icon.ico"

Write-Host "[1/5] Building release binary..."
Push-Location $RepoRoot
try {
  cargo build --release --manifest-path (Join-Path $RepoRoot "Cargo.toml")
} finally {
  Pop-Location
}

Write-Host "[2/5] Installing files..."
New-Item -Path $InstallDir -ItemType Directory -Force | Out-Null
Copy-Item (Join-Path $RepoRoot "target\release\fivepoints_calc.exe") $ExePath -Force
Copy-Item (Join-Path $RepoRoot "assets\icons\fivepoints_icon.ico") $IconPath -Force

$cmdText = @'
@echo off
setlocal

if "%~1"=="" (
  echo Usage: fivepoints_calc.exe --ifile DATA --skd32m SCHEDULE --skd34m SCHEDULE --frequency C^|X [--output-dir DIR]
  exit /b 1
) else (
  "%~dp0fivepoints_calc.exe" %*
)

echo.
pause
'@
Set-Content -Path $CmdPath -Value $cmdText -Encoding ASCII

Write-Host "[3/5] Creating shortcuts..."
$wshell = New-Object -ComObject WScript.Shell

$desktopShortcut = Join-Path $env:USERPROFILE "Desktop\FivePointsCalc.lnk"
$startMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$startShortcut = Join-Path $startMenuDir "FivePointsCalc.lnk"

foreach ($shortcutPath in @($desktopShortcut, $startShortcut)) {
  $shortcut = $wshell.CreateShortcut($shortcutPath)
  $shortcut.TargetPath = $CmdPath
  $shortcut.WorkingDirectory = $InstallDir
  $shortcut.IconLocation = "$IconPath,0"
  $shortcut.Save()
}

Write-Host "[4/5] Updating PATH for current user (if needed)..."
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$segments = @()
if ($userPath) { $segments = $userPath -split ";" }
if (-not ($segments -contains $InstallDir)) {
  $newPath = ($segments + $InstallDir) -join ";"
  [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
  Write-Host "Added to user PATH: $InstallDir"
} else {
  Write-Host "User PATH already contains: $InstallDir"
}

Write-Host "[5/5] Done."
Write-Host "Installed:"
Write-Host "  Binary   : $ExePath"
Write-Host "  Launcher : $CmdPath"
Write-Host "  Shortcut : $desktopShortcut"
Write-Host "  Shortcut : $startShortcut"
