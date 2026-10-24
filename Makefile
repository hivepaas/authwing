# Building authwing in one word: `make build`, `make run`; `make` alone lists
# the rest. Needs Rust (rustup), plus each platform's tools
# (docs/platforms.md). On Windows, run it from Git Bash or MSYS2.
#
# The desktop app does not cross-compile (GPUI links each system's own window
# libraries), so build-linux runs on Linux and build-windows on Windows;
# CI builds all three (Actions > CI > Run workflow). Binaries land in dist/.

# rustup's tools, for shells that have not sourced ~/.cargo/env. Make 3.81
# (macOS) looks commands up on the PATH it started with, so they are named by
# path too.
CARGO_BIN := $(HOME)/.cargo/bin
export PATH := $(CARGO_BIN):$(PATH)

CARGO ?= $(if $(wildcard $(CARGO_BIN)/cargo),$(CARGO_BIN)/cargo,cargo)
RUSTUP ?= $(if $(wildcard $(CARGO_BIN)/rustup),$(CARGO_BIN)/rustup,rustup)
DIST := dist
# Linux window systems; `make build-linux LINUX_FEATURES=wayland` without the X11 libraries.
LINUX_FEATURES ?= wayland,x11

ifeq ($(OS),Windows_NT)
HOST := Windows
else
HOST := $(shell uname -s)
endif
ARCH := $(shell uname -m 2>/dev/null)

.DEFAULT_GOAL := help
.PHONY: help build run dev build-mac build-linux build-windows test lint fmt check \
	install-desktop package doctor clean

help: ## List the targets
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F ':.*## ' '{printf "  make %-20s %s\n", $$1, $$2}'

## The desktop app

build: ## Release build for this computer (build-mac, build-linux or build-windows)
ifeq ($(HOST),Darwin)
	@$(MAKE) --no-print-directory build-mac
else ifeq ($(HOST),Linux)
	@$(MAKE) --no-print-directory build-linux
else ifeq ($(HOST),Windows)
	@$(MAKE) --no-print-directory build-windows
else
	@echo "no desktop build for $(HOST)"; exit 1
endif

run: ## Build (debug) and start the app
	$(CARGO) run -p authwing

dev: ## Debug build: quicker to make, slower to run
	$(CARGO) build -p authwing

build-mac: ## macOS release build -> dist/authwing-macos-<arch>
ifneq ($(HOST),Darwin)
	@echo "build-mac runs on a Mac (this is $(HOST)); CI: Run workflow > macos"; exit 1
endif
	$(CARGO) build --release -p authwing
	@mkdir -p $(DIST) && cp target/release/authwing $(DIST)/authwing-macos-$(ARCH)
	@echo "built $(DIST)/authwing-macos-$(ARCH)"

build-linux: ## Linux release build -> dist/authwing-linux-<arch>
ifneq ($(HOST),Linux)
	@echo "build-linux runs on Linux (this is $(HOST)): GPUI links the system's Wayland and X11"
	@echo "libraries. Build in a Linux VM or container, or CI: Run workflow > linux."; exit 1
endif
	$(CARGO) build --release -p authwing --no-default-features --features $(LINUX_FEATURES)
	@mkdir -p $(DIST) && cp target/release/authwing $(DIST)/authwing-linux-$(ARCH)
	@echo "built $(DIST)/authwing-linux-$(ARCH)"

build-windows: ## Windows release build -> dist/authwing-windows-<arch>.exe
ifneq ($(HOST),Windows)
	@echo "build-windows runs on Windows (this is $(HOST)), with Visual Studio Build Tools;"
	@echo "see docs/platforms.md, or CI: Run workflow > windows."; exit 1
endif
	$(CARGO) build --release -p authwing
	@mkdir -p $(DIST) && cp target/release/authwing.exe $(DIST)/authwing-windows-$(ARCH).exe
	@echo "built $(DIST)/authwing-windows-$(ARCH).exe"

install-desktop: ## Linux: add the built app to the desktop's launcher
	scripts/install-desktop.sh

package: build ## What a release ships for this computer (docs/releasing.md) -> dist/
ifeq ($(HOST),Darwin)
	scripts/package_macos.sh
else ifeq ($(HOST),Linux)
	scripts/package_linux.sh
else
	scripts/package_windows.sh
endif

## Before a pull request (CONTRIBUTING.md)

fmt: ## Format the code
	$(CARGO) fmt --all

lint: ## Formatting and clippy, as CI checks them
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace --all-targets -- -D warnings

test: ## All tests
	$(CARGO) test --workspace

check: lint test ## lint, then test

## Housekeeping

doctor: ## Which tools are here, and which are missing
	@printf "%-12s " cargo; $(CARGO) --version 2>/dev/null || echo "missing: https://rustup.rs"
	@printf "%-12s " cmake; $${CMAKE:-cmake} --version 2>/dev/null | head -1 | grep . || echo "missing, or does not run here (voice builds whisper.cpp with it): see docs/platforms.md"

clean: ## Remove build output
	$(CARGO) clean
	rm -rf $(DIST)
