#!/usr/bin/env bash
set -euo pipefail

APP_PATH="${1:?usage: codesign-app.sh <app-path>}"

if [[ -z "${APPLE_SIGNING_IDENTITY:-}" ]]; then
    echo "APPLE_SIGNING_IDENTITY not set, skipping codesign"
    exit 0
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

codesign --force --options runtime --timestamp \
    --entitlements "$SCRIPT_DIR/entitlements.plist" \
    --sign "$APPLE_SIGNING_IDENTITY" \
    "$APP_PATH"

codesign --verify --strict --verbose=2 "$APP_PATH"
