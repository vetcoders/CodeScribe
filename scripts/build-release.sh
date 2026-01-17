#!/bin/bash
# Build CodeScribe.app with bundled CLI sidecar
# Created by M&K (c)2026 VetCoders

set -e

echo "=== Building CodeScribe Release ==="

# Get target triple
TARGET=$(rustc -vV | grep host | cut -d' ' -f2)
echo "Target: $TARGET"

# 1. Build CLI (release)
echo ""
echo ">>> Building codescribe (CLI engine)..."
cargo build --release -p codescribe

# 2. Copy with target triple suffix for Tauri sidecar
CLI_BIN="target/release/codescribe"
SIDECAR_BIN="target/release/codescribe-${TARGET}"

echo ">>> Creating sidecar: $SIDECAR_BIN"
cp "$CLI_BIN" "$SIDECAR_BIN"

# 3. Build Tauri app
echo ""
echo ">>> Building CodeScribe.app (Tauri)..."
cd tauri-app
cargo tauri build

echo ""
echo "=== Build Complete ==="
echo "App: target/release/bundle/macos/CodeScribe.app"
