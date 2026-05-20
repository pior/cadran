#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$REPO_ROOT"

BINARY="${CADRAN_BINARY:-target/release/cadran}"
APP_OUT="$REPO_ROOT/target/Cadran.app"

# cargo pkgid output: "path+file:///.../cadran@0.1.0" (cargo 1.77+) or "...#0.1.0" (older)
VERSION="$(cargo pkgid | sed 's/.*[#@]//')"

if [[ ! -x "$BINARY" ]]; then
    echo "Error: binary not found or not executable at $BINARY" >&2
    echo "Build with: cargo build --release [--target <arch>-apple-darwin]" >&2
    exit 1
fi

echo "Bundling Cadran.app version $VERSION (binary: $BINARY)"

"$SCRIPT_DIR/generate-icons.sh"

rm -rf "$APP_OUT"
mkdir -p "$APP_OUT/Contents/MacOS"
mkdir -p "$APP_OUT/Contents/Resources"

cp "$BINARY" "$APP_OUT/Contents/MacOS/cadran"
cp "$REPO_ROOT/Info.plist" "$APP_OUT/Contents/Info.plist"

plutil -replace CFBundleVersion -string "$VERSION" "$APP_OUT/Contents/Info.plist"
plutil -replace CFBundleShortVersionString -string "$VERSION" "$APP_OUT/Contents/Info.plist"
plutil -lint "$APP_OUT/Contents/Info.plist" >/dev/null

cp "$REPO_ROOT/target/AppIcon.icns" "$APP_OUT/Contents/Resources/"

echo "Built $APP_OUT"
