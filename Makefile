.PHONY: all release install clean dev test-all cov help bundle bundle-app bundle-icon

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

## cov: Run tests and generate coverage report (HTML)
cov:
	cargo tarpaulin --workspace --out Html

## install: Build and deploy binaries using install.sh
install:
	./install.sh

## bundle: Create a macOS app bundle for jki-agent
bundle: release bundle-icon bundle-app

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

## clean: Remove build artifacts
clean:
	cargo clean
	rm -rf $(TARGET_DIR)/*.icns
	rm -rf $(TARGET_DIR)/*.app

## help: Show this help message
help:
	@echo "Just Keep Identity (jki) Build & Deploy Tool"
	@echo ""
	@echo "Usage:"
	@echo "  make [target]"
	@echo ""
	@echo "Targets:"
	@grep -E '^##' Makefile | sed -e 's/## //g' | column -t -s ':'
