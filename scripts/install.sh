#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OS="$(uname -s)"

case "${OS}" in
  Linux)
    exec "${SCRIPT_DIR}/install_linux.sh"
    ;;
  Darwin)
    exec "${SCRIPT_DIR}/install_macos.sh"
    ;;
  *)
    echo "Unsupported OS for this script: ${OS}" >&2
    echo "Use scripts/install_windows.ps1 on Windows." >&2
    exit 1
    ;;
esac
