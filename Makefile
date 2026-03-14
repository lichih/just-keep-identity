# Load macOS signing variables if the file exists
-include .env.macos

.PHONY: all release install clean dev test-all cov cov-audit snapshot snapshot-clean help bundle bundle-app bundle-icon

# Directories
BIN_DIR = $(HOME)/.local/bin
TARGET_DIR = target/release
APP_NAME = jki-agent
APP_BUNDLE = $(TARGET_DIR)/$(APP_NAME).app
APP_CONTENTS = $(APP_BUNDLE)/Contents
APP_MACOS = $(APP_CONTENTS)/MacOS
APP_RESOURCES = $(APP_CONTENTS)/Resources
ICON_SET = $(TARGET_DIR)/icon.iconset

# Binaries
CORE_BINS = jki jkim
AGENT_BINS = jki-agent

all: help

## release: Build all release binaries (Core + Agent)
release:
	cargo build --release --workspace

## release-core: Build only core release binaries (jki, jkim)
release-core:
	cargo build --release -p jki -p jkim

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

## cov: Run tests and generate coverage report (HTML)
cov:
	CARGO_TARGET_DIR=target/tarpaulin cargo tarpaulin --workspace --all-features --engine llvm --all-targets --out Html --skip-clean

## cov-audit: Show uncovered line numbers using jq (Run 'make cov' first)
cov-audit:
	@if [ ! -f tarpaulin-report.json ]; then \
		echo "Error: tarpaulin-report.json not found. Generating it now..."; \
		CARGO_TARGET_DIR=target/tarpaulin cargo tarpaulin --workspace --all-features --engine llvm --all-targets --out Json --skip-clean --output-dir .; \
	fi
	@echo "--- Uncovered Lines Audit ---"
	@jq -r '.files[] | {path: (.path | join("/")), uncovered: [.traces[] | select(.stats.Line == 0) | .line] | sort} | select(.uncovered | length > 0) | "\(.path): \(.uncovered | join(", "))"' tarpaulin-report.json

## snapshot: Create .stable snapshots for all Rust source files
snapshot:
	@echo "Creating .stable snapshots..."
	@find crates -name "*.rs" -exec cp {} {}.stable \;
	@echo "Snapshots created."

## snapshot-clean: Remove all .stable snapshot files
snapshot-clean:
	@echo "Cleaning up .stable snapshots..."
	@find crates -name "*.stable" -delete
	@echo "Cleanup complete."

## install: Build and deploy ALL binaries using install.sh
install:
	./install.sh

## install-core: Build and deploy ONLY CORE binaries (jki, jkim)
install-core:
	./install.sh --core-only

## bundle: Create a macOS app bundle for jki-agent
bundle: release bundle-icon bundle-app

## demo: Record a demo GIF using VHS (Requires vhs installed)
demo:
	@echo "Recording demo using mock data..."
	@vhs < docs/demo.tape
	@echo "Demo recorded to docs/assets/demo.gif"

bundle-icon:
	@echo "Creating icon.icns..."
	@mkdir -p $(ICON_SET)
	@sips -z 16 16     crates/jki-agent/assets/icon.png --out $(ICON_SET)/icon_16x16.png > /dev/null
	@sips -z 32 32     crates/jki-agent/assets/icon.png --out $(ICON_SET)/icon_16x16@2x.png > /dev/null
	@sips -z 32 32     crates/jki-agent/assets/icon.png --out $(ICON_SET)/icon_32x32.png > /dev/null
	@sips -z 64 64     crates/jki-agent/assets/icon.png --out $(ICON_SET)/icon_32x32@2x.png > /dev/null
	@sips -z 128 128   crates/jki-agent/assets/icon.png --out $(ICON_SET)/icon_128x128.png > /dev/null
	@sips -z 256 256   crates/jki-agent/assets/icon.png --out $(ICON_SET)/icon_128x128@2x.png > /dev/null
	@sips -z 256 256   crates/jki-agent/assets/icon.png --out $(ICON_SET)/icon_256x256.png > /dev/null
	@sips -z 512 512   crates/jki-agent/assets/icon.png --out $(ICON_SET)/icon_256x256@2x.png > /dev/null
	@sips -z 512 512   crates/jki-agent/assets/icon.png --out $(ICON_SET)/icon_512x512.png > /dev/null
	@sips -z 1024 1024 crates/jki-agent/assets/icon.png --out $(ICON_SET)/icon_512x512@2x.png > /dev/null
	@iconutil -c icns $(ICON_SET)
	@mv $(TARGET_DIR)/icon.icns $(TARGET_DIR)/icon.icns
	@rm -rf $(ICON_SET)

bundle-app:
	@echo "Bundling $(APP_NAME).app..."
	@mkdir -p $(APP_MACOS)
	@mkdir -p $(APP_RESOURCES)
	@cp $(TARGET_DIR)/$(APP_NAME) $(APP_MACOS)/
	@cp crates/jki-agent/Info.plist $(APP_CONTENTS)/
	@cp $(TARGET_DIR)/icon.icns $(APP_RESOURCES)/
	@echo "Bundle created at $(APP_BUNDLE)"

## sign: Codesign the macOS app bundle
sign: bundle
	./scripts/sign_macos.sh "$(APP_BUNDLE)" "$(SIGNING_IDENTITY)"

## notarize: Notarize the macOS app bundle
notarize: sign
	./scripts/notarize_macos.sh "$(APP_BUNDLE)" "$(APPLE_ID)" "$(TEAM_ID)" "$(AC_PASSWORD)"

## brew-package: Package binaries for Homebrew (macOS ARM64)
brew-package: release
	@echo "Packaging binaries for Homebrew..."
	@mkdir -p target/brew
	@cp $(TARGET_DIR)/jki target/brew/
	@cp $(TARGET_DIR)/jkim target/brew/
	@cp $(TARGET_DIR)/jki-agent target/brew/
	@tar -czf target/jki-macos-arm64.tar.gz -C target/brew .
	@rm -rf target/brew
	@echo "Package created at target/jki-macos-arm64.tar.gz"
	@shasum -a 256 target/jki-macos-arm64.tar.gz

## brew-dist: Upload package to GitHub release and update Homebrew formula
brew-dist: brew-package
	@echo "Uploading to GitHub Release..."
	@gh release upload $(shell git describe --tags --abbrev=0) target/jki-macos-arm64.tar.gz --clobber
	@echo "Update docs/homebrew-jki.rb with the new SHA256 above."

## clean: Remove build artifacts
clean:
	cargo clean
	rm -rf $(TARGET_DIR)/*.icns
	rm -rf $(TARGET_DIR)/*.app
	rm -f target/jki-macos-arm64.tar.gz

## help: Show this help message
help:
	@echo "Just Keep Identity (jki) Build & Deploy Tool"
	@echo ""
	@echo "Usage:"
	@echo "  make [target]"
	@echo ""
	@echo "Targets:"
	@grep -E '^##' Makefile | sed -e 's/## //g' | column -t -s ':'
