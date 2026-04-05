#!/bin/bash
# Smoke-test a built CodeScribe DMG by mounting it and validating bundle contents.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
APP_NAME="${CODESCRIBE_APP_NAME:-CodeScribe}"
BUNDLE_ID="${CODESCRIBE_BUNDLE_ID:-com.codescribe.app}"
DMG_PATH="${1:-}"

if [[ -z "$DMG_PATH" ]]; then
  DMG_PATH="$(ls -t "$ROOT_DIR"/CodeScribe_*.dmg 2>/dev/null | head -n 1 || true)"
fi

if [[ -z "$DMG_PATH" ]]; then
  echo "ERROR: No DMG specified and no CodeScribe_*.dmg found in $ROOT_DIR" >&2
  exit 1
fi

if [[ ! -f "$DMG_PATH" ]]; then
  echo "ERROR: DMG not found: $DMG_PATH" >&2
  exit 1
fi

MOUNT_DIR="$(mktemp -d)"
DEVICE=""

cleanup() {
  if [[ -n "$DEVICE" ]]; then
    hdiutil detach "$DEVICE" -quiet >/dev/null 2>&1 || true
  elif [[ -d "$MOUNT_DIR" ]]; then
    hdiutil detach "$MOUNT_DIR" -quiet >/dev/null 2>&1 || true
  fi
  rmdir "$MOUNT_DIR" >/dev/null 2>&1 || true
}

trap cleanup EXIT

echo "=== Smoke DMG ==="
echo "DMG: $DMG_PATH"

attach_output="$(hdiutil attach "$DMG_PATH" -nobrowse -readonly -mountpoint "$MOUNT_DIR" 2>&1)"
DEVICE="$(printf '%s\n' "$attach_output" | awk '/^\/dev\// { print $1; exit }')"

APP_PATH="$MOUNT_DIR/$APP_NAME.app"
PLIST_PATH="$APP_PATH/Contents/Info.plist"
BIN_DIR="$APP_PATH/Contents/MacOS"

[[ -d "$APP_PATH" ]] || {
  echo "ERROR: Mounted DMG does not contain $APP_NAME.app" >&2
  exit 1
}
[[ -L "$MOUNT_DIR/Applications" ]] || {
  echo "ERROR: Mounted DMG does not expose /Applications shortcut" >&2
  exit 1
}
[[ -f "$PLIST_PATH" ]] || {
  echo "ERROR: Missing Info.plist in mounted app bundle" >&2
  exit 1
}

/usr/bin/plutil -lint "$PLIST_PATH" >/dev/null

bundle_name="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleName' "$PLIST_PATH")"
bundle_identifier="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$PLIST_PATH")"
bundle_executable="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$PLIST_PATH")"

[[ "$bundle_name" == "$APP_NAME" ]] || {
  echo "ERROR: Expected CFBundleName=$APP_NAME, got $bundle_name" >&2
  exit 1
}
[[ "$bundle_identifier" == "$BUNDLE_ID" ]] || {
  echo "ERROR: Expected CFBundleIdentifier=$BUNDLE_ID, got $bundle_identifier" >&2
  exit 1
}

main_bin="$BIN_DIR/$bundle_executable"
loop_bin="$BIN_DIR/codescribe-loop"
quality_bin="$BIN_DIR/codescribe-quality"

for bin_path in "$main_bin" "$loop_bin" "$quality_bin"; do
  [[ -x "$bin_path" ]] || {
    echo "ERROR: Expected executable in bundle: $bin_path" >&2
    exit 1
  }
done

"$main_bin" --version >/dev/null
"$loop_bin" --help >/dev/null
"$quality_bin" --help >/dev/null

echo "Smoke check passed: mounted DMG contains a runnable CodeScribe bundle."
