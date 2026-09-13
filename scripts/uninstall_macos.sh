#!/usr/bin/env bash
set -euo pipefail

BIN_NAME="fivepoints_calc"
HELPER_NAME="fivepoints-calc"
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"

BIN_DIR="${INSTALL_PREFIX}/bin"
APP_BUNDLE="${HOME}/Applications/FivePointsCalc.app"

echo "[1/3] Removing app bundle..."
rm -rf "${APP_BUNDLE}"

echo "[2/3] Removing binaries..."
rm -f "${BIN_DIR}/${BIN_NAME}" "${BIN_DIR}/${HELPER_NAME}"

echo "[3/3] Done."
echo "Removed:"
echo "  ${APP_BUNDLE}"
echo "  ${BIN_DIR}/${BIN_NAME}"
echo "  ${BIN_DIR}/${HELPER_NAME}"
