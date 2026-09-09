set shell := ["zsh", "-cu"]

default:
    @just --list

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

typecheck:
    cargo check --locked --all-targets

lint:
    cargo clippy --locked --all-targets -- -D warnings

# Pass a test filter or --test <target> to run only the affected tests locally.
test *args:
    cargo test --locked {{args}} -- --test-threads=1

check-public:
    python3 tooling/check_public.py

test-public:
    python3 -m unittest discover -s tooling -p 'test_check_public.py'

check: fmt-check check-public lint test

build:
    cargo build --locked

release:
    cargo build --release --locked

install:
    cargo install --path . --locked

# Debug binary into ~/.local/bin (herduck-dev data). Re-sign after copy;
# macOS kills an unsigned/invalid adhoc Mach-O with `zsh: killed`.
install-debug:
    cargo build --locked
    mkdir -p ~/.local/bin
    cp target/debug/herduck ~/.local/bin/herduck
    codesign --force --sign - ~/.local/bin/herduck

# Install the debug binary and make a running server actually use it.
#
# Replacing the file on disk does not change the running server: it keeps executing the old
# image, so a new client meets an old server and the handshake dies with
# "lost connection to server". `live-handoff` passes the live PTY file descriptors to a server
# started from the new binary, so panes keep running across the swap. A failed handoff restores
# the old server's sockets, so the worst case is "still on the old build", not "panes lost".

# Install the debug binary and live-hand-off a running server onto it (panes survive).
install-debug-live: install-debug
    #!/usr/bin/env zsh
    set -euo pipefail
    # Clear inherited socket overrides: a pane started by the running server exports them, which
    # would point this command back at whichever server spawned the shell.
    running=$(env -u HERDUCK_SOCKET_PATH -u HERDUCK_CLIENT_SOCKET_PATH \
        -u ORK3_SOCKET_PATH -u ORK3_CLIENT_SOCKET_PATH \
        -u HERDR_SOCKET_PATH -u HERDR_CLIENT_SOCKET_PATH \
        herduck status --json 2>/dev/null | grep -o '"running":true' || true)
    if [[ -n "$running" ]]; then
        echo "handing live panes to the new binary…"
        env -u HERDUCK_SOCKET_PATH -u HERDUCK_CLIENT_SOCKET_PATH \
            -u ORK3_SOCKET_PATH -u ORK3_CLIENT_SOCKET_PATH \
            -u HERDR_SOCKET_PATH -u HERDR_CLIENT_SOCKET_PATH \
            herduck server live-handoff --import-exe ~/.local/bin/herduck
    else
        echo "no server running; the next \`herduck\` starts the new binary."
    fi
