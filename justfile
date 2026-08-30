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
    running=$(env -u HERDR_SOCKET_PATH -u HERDR_CLIENT_SOCKET_PATH \
        ork3 status --json 2>/dev/null | grep -o '"running":true' || true)
    if [[ -n "$running" ]]; then
        echo "handing live panes to the new binary…"
        env -u HERDR_SOCKET_PATH -u HERDR_CLIENT_SOCKET_PATH \
            ork3 server live-handoff --import-exe ~/.local/bin/ork3
    else
        echo "no server running; the next \`ork3\` starts the new binary."
    fi
