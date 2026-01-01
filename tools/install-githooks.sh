#!/usr/bin/env bash
set -euo pipefail

# Install git hooks for CodeScribe
# Usage: ./tools/install-githooks.sh

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOOKS_DIR="$ROOT/.git/hooks"

if [[ ! -d "$HOOKS_DIR" ]]; then
  echo "No .git/hooks directory found. Are you in a git repo?" >&2
  exit 1
fi

# Install pre-commit hook
ln -sf "$ROOT/tools/githooks/pre-commit" "$HOOKS_DIR/pre-commit"
chmod +x "$HOOKS_DIR/pre-commit"

# Install pre-push hook
ln -sf "$ROOT/tools/githooks/pre-push" "$HOOKS_DIR/pre-push"
chmod +x "$HOOKS_DIR/pre-push"

echo "Installed git hooks:"
ls -la "$HOOKS_DIR"/pre-commit
ls -la "$HOOKS_DIR"/pre-push

echo ""
echo "Git hooks installed successfully!"
echo ""
echo "Hooks will run automatically on:"
echo "  pre-commit:"
echo "    - Python: Ruff format + check (auto-fix)"
echo "    - Rust:   cargo fmt + cargo check"
echo "  pre-push:"
echo "    - Python: Ruff check"
echo "    - Rust:   fmt --check + clippy -D warnings + tests + release build"
echo "    - Security: Semgrep scan (if installed)"
