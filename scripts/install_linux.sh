#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

BIN_NAME="fivepoints_calc"
APP_ID="fivepoints-calc"
APP_NAME="FivePointsCalc"

INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"
BIN_DIR="${INSTALL_PREFIX}/bin"
ICON_DIR="${INSTALL_PREFIX}/share/icons/hicolor/256x256/apps"
APP_DIR="${INSTALL_PREFIX}/share/applications"

echo "[1/4] Building release binary..."
cargo build --release --manifest-path "${REPO_ROOT}/Cargo.toml"

echo "[2/4] Installing binary..."
mkdir -p "${BIN_DIR}"
install -m 0755 "${REPO_ROOT}/target/release/${BIN_NAME}" "${BIN_DIR}/${BIN_NAME}"

cat > "${BIN_DIR}/${APP_ID}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: fivepoints_calc --ifile DATA --skd32m SCHEDULE --skd34m SCHEDULE --frequency C|X [--output-dir DIR]"
  exit 1
fi

bin_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "${bin_dir}/fivepoints_calc" "$@"
EOF
chmod +x "${BIN_DIR}/${APP_ID}"

echo "[3/4] Installing icon and desktop entry..."
mkdir -p "${ICON_DIR}" "${APP_DIR}"
install -m 0644 "${REPO_ROOT}/assets/icons/fivepoints_icon_256.png" \
  "${ICON_DIR}/${APP_ID}.png"
cat > "${APP_DIR}/${APP_ID}.desktop" <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=${APP_NAME}
Comment=Five-point Gaussian fit calculator
TryExec=${BIN_DIR}/${BIN_NAME}
Exec=${BIN_DIR}/${BIN_NAME}
Icon=${APP_ID}
Terminal=true
StartupNotify=true
Categories=Science;Utility;
EOF

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${APP_DIR}" >/dev/null 2>&1 || true
fi

echo "[4/4] Done."
echo "Installed:"
echo "  Binary : ${BIN_DIR}/${BIN_NAME}"
echo "  Wrapper: ${BIN_DIR}/${APP_ID}"
echo "  Desktop: ${APP_DIR}/${APP_ID}.desktop"
echo
echo "If needed, add this to PATH:"
echo "  export PATH=\"${BIN_DIR}:\$PATH\""
