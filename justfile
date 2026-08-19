set shell := ["zsh", "-cu"]

default:
    @just --list

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

check:
    cargo fmt --all -- --check
    cargo clippy --locked --all-targets -- -D warnings
    cargo test --locked -- --test-threads=1

build:
    cargo build --locked

release:
    cargo build --release --locked

install:
    cargo install --path . --locked

# Debug binary into ~/.local/bin (ork3-dev data). Re-sign after copy;
# macOS kills an unsigned/invalid adhoc Mach-O with `zsh: killed`.
install-debug:
    cargo build --locked
    cp target/debug/ork3 ~/.local/bin/ork3
    codesign --force --sign - ~/.local/bin/ork3
