.PHONY: all release install clean dev test-all help

# Directories
BIN_DIR = $(HOME)/.local/bin
TARGET_DIR = target/release

# Binaries
BINS = jki jkim jki-agent

all: help

## release: Build release binaries
release:
	cargo build --release --workspace

## dev: Build debug binaries or run cargo watch (if installed)
dev:
	@if command -v cargo-watch >/dev/null; then \
		cargo watch -x build; \
	else \
		cargo build; \
	fi

## test-all: Run all tests in the workspace
test-all:
	cargo test --workspace

## install: Build and deploy binaries using install.sh
install:
	./install.sh

## clean: Remove build artifacts
clean:
	cargo clean

## help: Show this help message
help:
	@echo "Just Keep Identity (jki) Build & Deploy Tool"
	@echo ""
	@echo "Usage:"
	@echo "  make [target]"
	@echo ""
	@echo "Targets:"
	@grep -E '^##' Makefile | sed -e 's/## //g' | column -t -s ':'
