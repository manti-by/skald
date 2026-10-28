# Makefile for skald.
#
# Usage:
#   make build   compile the binary into target/debug/skald
#   make run     run the player
#   make test    run the test suite
#   make check   fmt, clippy and tests
#   make release run the gate, then build target/release/skald
#   make install install the binary into CARGO_HOME/bin
#   make clean   remove build artifacts

CARGO ?= cargo
BIN := target/debug/skald
RELEASE_BIN := target/release/skald

.DEFAULT_GOAL := build
.PHONY: build run test check fmt fmt-check lint clean install release

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

## clean: remove build artifacts.
clean:
	$(CARGO) clean
