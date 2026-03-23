set shell := ["bash", "-c"]

target := `rustc -vV | sed -n 's/^host: //p'`
dist := "dist"

[private]
default:
    @just --list

# Build a debug binary
build:
    cargo build

# Build and run (pass args after --: `just run -- cluster list`)
run *args:
    cargo run -- {{ args }}

# Run all tests
test:
    cargo test

# Run clippy lints
lint:
    cargo clippy -- -D warnings

# Format source code
fmt:
    cargo fmt

# Check formatting without modifying files
fmt-check:
    cargo fmt -- --check

# Run lint + fmt-check (CI gate)
check: fmt-check lint
    cargo check

# Build a stripped, optimised release binary for the current host
release: _dist-dir
    cargo build --release --target {{ target }}
    just _strip-and-copy {{ target }}
    @echo "Binary → {{ dist }}/kubelima-{{ target }}"

# Build a release binary for a specific target triple
release-target triple: _dist-dir
    cargo build --release --target {{ triple }}
    just _strip-and-copy {{ triple }}
    @echo "Binary → {{ dist }}/kubelima-{{ triple }}"

[private]
_strip-and-copy triple:
    #!/usr/bin/env bash
    set -euo pipefail
    src="target/{{ triple }}/release/kubelima"
    dst="{{ dist }}/kubelima-{{ triple }}"
    cp "$src" "$dst"
    if command -v strip &>/dev/null; then
        strip "$dst"
        echo "Stripped debug symbols"
    fi
    if command -v upx &>/dev/null; then
        upx --best --lzma "$dst"
        echo "Compressed with UPX"
    fi
    ls -lh "$dst"

[private]
_dist-dir:
    mkdir -p {{ dist }}

# Remove build artefacts and dist/
clean:
    cargo clean
    rm -rf {{ dist }}

# Print current version from Cargo.toml
version:
    @cargo metadata --no-deps --format-version 1 | \
        python3 -c "import sys,json; d=json.load(sys.stdin); print(d['packages'][0]['version'])"
