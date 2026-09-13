#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

BIN_NAME="fivepoints_calc"
APP_NAME="FivePointsCalc"
APP_ID="jp.akimoto.fivepointscalc"

INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"
BIN_DIR="${INSTALL_PREFIX}/bin"
APP_BUNDLE="${HOME}/Applications/${APP_NAME}.app"
CONTENTS_DIR="${APP_BUNDLE}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"

echo "[1/5] Building release binary..."
cargo build --release --manifest-path "${REPO_ROOT}/Cargo.toml"

echo "[2/5] Installing CLI binary..."
mkdir -p "${BIN_DIR}"
install -m 0755 "${REPO_ROOT}/target/release/${BIN_NAME}" "${BIN_DIR}/${BIN_NAME}"

echo "[3/5] Creating app bundle..."
mkdir -p "${MACOS_DIR}" "${RESOURCES_DIR}"

cat > "${MACOS_DIR}/${APP_NAME}" <<EOF
#!/usr/bin/env bash
set -euo pipefail

if [[ \$# -gt 0 ]]; then
  exec "${BIN_DIR}/${BIN_NAME}" "\$@"
fi

if [[ -t 1 ]]; then
  echo "Usage: ${BIN_NAME} --ifile DATA --skd32m SCHEDULE --skd34m SCHEDULE --frequency C|X [--output-dir DIR]"
  exit 1
fi

/usr/bin/osascript <<OSA
tell application "Terminal"
  activate
  do script "${BIN_DIR}/${BIN_NAME}"
end tell
OSA
EOF
chmod +x "${MACOS_DIR}/${APP_NAME}"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT
ICONSET_DIR="${TMP_DIR}/fivepoints_icon.iconset"
mkdir -p "${ICONSET_DIR}"
ICON_SRC="${REPO_ROOT}/assets/icons/fivepoints_icon_1024.png"

if command -v sips >/dev/null 2>&1 && command -v iconutil >/dev/null 2>&1; then
  sips -z 16 16 "${ICON_SRC}" --out "${ICONSET_DIR}/icon_16x16.png" >/dev/null
  sips -z 32 32 "${ICON_SRC}" --out "${ICONSET_DIR}/icon_16x16@2x.png" >/dev/null
  sips -z 32 32 "${ICON_SRC}" --out "${ICONSET_DIR}/icon_32x32.png" >/dev/null
  sips -z 64 64 "${ICON_SRC}" --out "${ICONSET_DIR}/icon_32x32@2x.png" >/dev/null
  sips -z 128 128 "${ICON_SRC}" --out "${ICONSET_DIR}/icon_128x128.png" >/dev/null
  sips -z 256 256 "${ICON_SRC}" --out "${ICONSET_DIR}/icon_128x128@2x.png" >/dev/null
  sips -z 256 256 "${ICON_SRC}" --out "${ICONSET_DIR}/icon_256x256.png" >/dev/null
  sips -z 512 512 "${ICON_SRC}" --out "${ICONSET_DIR}/icon_256x256@2x.png" >/dev/null
  sips -z 512 512 "${ICON_SRC}" --out "${ICONSET_DIR}/icon_512x512.png" >/dev/null
  cp "${ICON_SRC}" "${ICONSET_DIR}/icon_512x512@2x.png"
  iconutil -c icns "${ICONSET_DIR}" -o "${RESOURCES_DIR}/fivepoints_icon.icns"
else
  cp "${REPO_ROOT}/assets/icons/fivepoints_icon_512.png" "${RESOURCES_DIR}/fivepoints_icon.png"
fi

cat > "${CONTENTS_DIR}/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleDisplayName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleIdentifier</key>
  <string>${APP_ID}</string>
  <key>CFBundleVersion</key>
  <string>1.0.0</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0.0</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleExecutable</key>
  <string>${APP_NAME}</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>CFBundleIconFile</key>
  <string>fivepoints_icon</string>
</dict>
</plist>
EOF

echo "[4/5] Writing helper launcher..."
cat > "${BIN_DIR}/fivepoints-calc" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
exec fivepoints_calc "$@"
EOF
chmod +x "${BIN_DIR}/fivepoints-calc"

echo "[5/5] Done."
echo "Installed:"
echo "  CLI : ${BIN_DIR}/${BIN_NAME}"
echo "  App : ${APP_BUNDLE}"
echo
echo "If needed, add this to PATH:"
echo "  export PATH=\"${BIN_DIR}:\$PATH\""
