#!/usr/bin/env bash
set -euo pipefail

APP_ID="fivepoints-calc"
BIN_NAME="fivepoints_calc"
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"

BIN_DIR="${INSTALL_PREFIX}/bin"
ICON_PATH="${INSTALL_PREFIX}/share/icons/hicolor/256x256/apps/${APP_ID}.png"
DESKTOP_PATH="${INSTALL_PREFIX}/share/applications/${APP_ID}.desktop"

echo "[1/3] Removing binaries..."
rm -f "${BIN_DIR}/${BIN_NAME}" "${BIN_DIR}/${APP_ID}"

echo "[2/3] Removing desktop/icon files..."
rm -f "${ICON_PATH}" "${DESKTOP_PATH}"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${INSTALL_PREFIX}/share/applications" >/dev/null 2>&1 || true
fi

echo "[3/3] Done."
echo "Removed:"
echo "  ${BIN_DIR}/${BIN_NAME}"
echo "  ${BIN_DIR}/${APP_ID}"
echo "  ${DESKTOP_PATH}"
echo "  ${ICON_PATH}"
