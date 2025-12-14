# CodeScribe Build System
# Speech-to-text tray app for macOS

.PHONY: all build install clean test check fmt clippy lint help run

# Default target
all: build

# Determine cargo bin dir
CARGO_BIN ?= $(if $(CARGO_HOME),$(CARGO_HOME)/bin,$(HOME)/.cargo/bin)
LOCKFILE ?= /tmp/codescribe-make.lock

# Build release
build:
	cargo build --release

# Build debug
build-debug:
	cargo build

# Install codescribe binary
# Lock prevents concurrent builds
install:
	@if [ -f "$(LOCKFILE)" ]; then \
		old_pid=$$(cat "$(LOCKFILE)" 2>/dev/null); \
		if [ -n "$$old_pid" ] && kill -0 "$$old_pid" 2>/dev/null; then \
			echo "Another build running (PID $$old_pid). Aborting."; \
			exit 1; \
		fi; \
		echo "Removing stale lock (PID $$old_pid dead)"; \
		rm -f "$(LOCKFILE)"; \
	fi
	@echo $$$$ > "$(LOCKFILE)"
	@trap 'rm -f $(LOCKFILE)' EXIT; \
	set -e; \
	cargo build --release; \
	mkdir -p "$(CARGO_BIN)"; \
	install -m 755 target/release/codescribe "$(CARGO_BIN)/codescribe"; \
	echo "Installed: codescribe → $(CARGO_BIN)"

# Run tests
test:
	cargo test

# Check compilation
check:
	cargo check

# Format code
fmt:
	cargo fmt

# Clippy with warnings as errors
clippy:
	cargo clippy -- -D warnings

# Full lint (fmt check + clippy)
lint:
	cargo fmt -- --check
	cargo clippy -- -D warnings

# Clean build artifacts
clean:
	cargo clean

# Remove stale build lock
unlock:
	@rm -f "$(LOCKFILE)" && echo "Lock removed" || echo "No lock"

# Run in foreground (debug build)
run:
	cargo run

# Run release build
run-release:
	cargo run --release

# Help
help:
	@echo "CodeScribe Build System"
	@echo ""
	@echo "Usage:"
	@echo "  make build       - Build release binary"
	@echo "  make build-debug - Build debug binary"
	@echo "  make install     - Build & install to ~/.cargo/bin"
	@echo "  make test        - Run tests"
	@echo "  make check       - Check compilation"
	@echo "  make fmt         - Format code"
	@echo "  make clippy      - Run clippy (warnings = errors)"
	@echo "  make lint        - Full lint (fmt check + clippy)"
	@echo "  make clean       - Clean build artifacts"
	@echo "  make run         - Run debug build"
	@echo "  make run-release - Run release build"
	@echo ""
	@echo "Quick start:"
	@echo "  make install     - Just works™"
