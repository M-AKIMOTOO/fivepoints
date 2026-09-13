Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\FivePointsCalc"
$DesktopShortcut = Join-Path $env:USERPROFILE "Desktop\FivePointsCalc.lnk"
$StartShortcut = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\FivePointsCalc.lnk"

Write-Host "[1/4] Removing shortcuts..."
Remove-Item -Path $DesktopShortcut -Force -ErrorAction SilentlyContinue
Remove-Item -Path $StartShortcut -Force -ErrorAction SilentlyContinue

Write-Host "[2/4] Removing installed files..."
Remove-Item -Path $InstallDir -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "[3/4] Removing PATH entry if present..."
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath) {
  $parts = $userPath -split ";" | Where-Object { $_ -and ($_ -ne $InstallDir) }
  $newPath = $parts -join ";"
  [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
}

Write-Host "[4/4] Done."
Write-Host "Removed:"
Write-Host "  $InstallDir"
Write-Host "  $DesktopShortcut"
Write-Host "  $StartShortcut"
