# Makefile for skald.
#
# Usage:
#   make build   compile the binary into target/debug/skald
#   make run     run the player
#   make test    run the test suite
#   make check   fmt, clippy and tests
#   make release run the gate, then build target/release/skald
#   make install install the binary into CARGO_HOME/bin
#   make app     run the gate, then bundle target/Skald.app
#   make icons   regenerate assets/icon.icns from media/skald.svg
#   make clean   remove build artifacts

CARGO ?= cargo
BIN := target/debug/skald
RELEASE_BIN := target/release/skald
APP_BUNDLE := target/Skald.app

.DEFAULT_GOAL := build
.PHONY: build run test check fmt fmt-check lint clean install release app bundle icons

## build: compile the binary.
build:
	$(CARGO) build

## run: launch the player.
run:
	$(CARGO) run

## test: run the unit tests.
test:
	$(CARGO) test

## check: formatting, lints and tests, the gate for committing.
check: fmt-check lint test

## fmt: format the source in place.
fmt:
	$(CARGO) fmt

## fmt-check: fail if the source is not formatted.
fmt-check:
	$(CARGO) fmt --check

## lint: run clippy over every target; must stay warning-free.
lint:
	$(CARGO) clippy --all-targets -- -D warnings

# `release` builds the optimized binary. It runs the same gate as `check` first,
# so an unformatted or unlinted tree cannot produce a release artifact.
release: check
	$(CARGO) build --release
	@ls -lh $(RELEASE_BIN)

## install: install the binary into CARGO_HOME/bin.
install:
	$(CARGO) install --path .

# `app` gates, builds the optimized binary and wraps it in a macOS .app whose
# Dock icon comes from media/skald.svg.
app: check
	$(CARGO) build --release
	./scripts/bundle.sh

# `bundle` is the packaging step alone: no gate, no rebuild.
bundle:
	$(CARGO) build --release
	./scripts/bundle.sh

## icons: regenerate assets/icon.icns from the icon source SVG.
icons:
	./scripts/make-icons.sh

## clean: remove build artifacts.
clean:
	$(CARGO) clean
	rm -rf $(APP_BUNDLE) icons.iconset
