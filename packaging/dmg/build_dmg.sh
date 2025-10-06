#!/bin/bash
# build_dmg.sh
#
# Purpose: Build a simple DMG for VistaScribe distribution.
# - Includes (if present): packaging/dist/VistaScribe.app (tray app)
# - Always includes helper scripts: Install Backend, Get Models, Uninstall Backend
# - Creates VistaScribe.dmg in packaging/dmg/
#
# Requirements: hdiutil (macOS), optional: create-dmg (not required)

set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname "$0")/../.." && pwd)"
OUT_DIR="$(cd -- "$(dirname "$0")" && pwd)"
STAGE_DIR="${OUT_DIR}/stage"
DMG_NAME="VistaScribe.dmg"

rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"

# Copy app if built
APP_SRC="${ROOT_DIR}/packaging/dist/VistaScribe.app"
if [[ -d "$APP_SRC" ]]; then
  echo "[i] Adding app bundle: $APP_SRC"
  cp -R "$APP_SRC" "$STAGE_DIR/Vista Scribe.app"
else
  echo "[!] App bundle not found at $APP_SRC — continuing without it."
  echo "    Build it first with: (cd packaging && python setup.py py2app)"
fi

# Applications symlink for drag-and-drop install UX
ln -sf /Applications "$STAGE_DIR/Applications"

# Minimal README inside DMG
cat >"$STAGE_DIR/README-INSTALL.txt" <<'TXT'
VistaScribe — Installation
==========================

1) Przeciągnij "Vista Scribe.app" do aliasu "Applications".
2) Otwórz aplikację z /Applications. Pierwsze uruchomienie:
   - pobierze/wykryje modele Whisper,
   - poprosi o uprawnienia (Microphone, Accessibility, Input Monitoring),
   - uruchomi tray + backend w tle i zapisze log do ~/Library/Logs/VistaScribe.app.log.

Nie musisz uruchamiać żadnych *.command — wszystko dzieje się w aplikacji.
TXT

# Create DMG
DMG_PATH="${OUT_DIR}/${DMG_NAME}"
rm -f "$DMG_PATH"
hdiutil create -volname "VistaScribe" -srcfolder "$STAGE_DIR" -ov -format UDZO "$DMG_PATH"

echo "[✓] Built DMG: $DMG_PATH"
