# Installation

This project is a Rust CLI app. The scripts below build a release binary and install it per OS.

## Prerequisites

- Rust toolchain with `cargo` available in PATH
- Build dependencies required by this project (`plotters`, `image`, etc. are fetched by Cargo)

## Linux

```bash
bash scripts/install_linux.sh
```

Installed by default under `~/.local`:

- `~/.local/bin/fivepoints_calc`
- `~/.local/bin/fivepoints-calc` (wrapper)
- `~/.local/share/applications/fivepoints-calc.desktop`
- `~/.local/share/icons/hicolor/256x256/apps/fivepoints-calc.png`

## macOS

```bash
bash scripts/install_macos.sh
```

Installed by default:

- `~/.local/bin/fivepoints_calc`
- `~/Applications/FivePointsCalc.app`

## Windows (PowerShell)

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install_windows.ps1
```

Installed by default:

- `%LOCALAPPDATA%\Programs\FivePointsCalc\fivepoints_calc.exe`
- `%LOCALAPPDATA%\Programs\FivePointsCalc\fivepoints_calc.cmd`
- Desktop and Start Menu shortcuts

## Windows MSI Build

Build MSI on a Windows build machine:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build_windows_msi.ps1
```

Build and immediately install MSI:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build_windows_msi.ps1 -Install
```

Output:

- `target\wix\*.msi`

Requirements on the build machine:

- Rust toolchain (`cargo`)
- WiX Toolset v3 (`candle.exe`, `light.exe` in PATH)
- `cargo-wix` (auto-installed by the script if missing)

## Auto-select script (Linux/macOS)

```bash
bash scripts/install.sh
```

## Uninstall

Linux/macOS auto-select:

```bash
bash scripts/uninstall.sh
```

Linux only:

```bash
bash scripts/uninstall_linux.sh
```

macOS only:

```bash
bash scripts/uninstall_macos.sh
```

Windows:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\uninstall_windows.ps1
```

## Run

Run the analyzer by giving the observation data, the two schedules, and the frequency:

```bash
fivepoints_calc \
  --ifile /path/to/observation.tsv \
  --skd32m /path/to/I26191F32.skd \
  --skd34m /path/to/I26191F34.skd \
  --frequency c
```

The optional output directory defaults to `./five_point_result`:

```bash
fivepoints_calc --ifile observation.tsv --skd32m I26191F32.skd --skd34m I26191F34.skd --frequency x --output-dir /path/to/output_dir
```

The command prints the schedule/data comparison and fit results, then saves the same report and PNG plots below the output directory.

## About Rust Requirement for Packaging

- End users do not need Rust when you distribute prebuilt packages/binaries.
- Rust is required on the build machine (CI or your dev machine) that creates those packages.
