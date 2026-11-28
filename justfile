# Pulse Sync task runner.
#
# `just` comes from the `rust-just` dev dependency, so after `pnpm install`
# run recipes as `pnpm <recipe>` (see package.json) or `pnpm just <recipe>`.
# A globally installed `just` works too.

set shell := ["bash", "-cu"]

data_dir := justfile_directory() / ".pulse-dev"

default:
    @just --list

# Run the daemon and the Vite dev server together (UI: http://localhost:5173).
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p "{{data_dir}}"
    trap 'kill 0' EXIT
    cargo run -p pulse-sync -- --data-dir "{{data_dir}}" &
    pnpm --dir web dev &
    wait

# Run only the daemon against the dev data directory.
daemon *args:
    mkdir -p "{{data_dir}}"
    cargo run -p pulse-sync -- --data-dir "{{data_dir}}" {{args}}

# Run only the web dev server.
web:
    pnpm --dir web dev

# Regenerate TypeScript types from Rust (web/src/api/generated).
gen-types:
    cargo test --workspace --quiet export_bindings

# Run all tests (this also regenerates TypeScript types).
test:
    cargo test --workspace
    pnpm --dir web test

# Formatting, clippy, biome and tsc: what CI runs.
lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    pnpm --dir web lint

# Apply formatters.
fmt:
    cargo fmt --all
    pnpm --dir web format

# Release build: a single `pulsed` binary with the web UI embedded.
build:
    pnpm --dir web build
    cargo build --release -p pulse-sync --features embed-web
    @echo "built target/release/pulsed"

# Delete the dev data directory (index, device key, API token).
reset-dev:
    rm -rf "{{data_dir}}"

# Zip the source (everything not git-ignored) into archives/.
archive *args:
    scripts/archive.sh {{args}}
