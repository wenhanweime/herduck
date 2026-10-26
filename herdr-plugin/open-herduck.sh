#!/usr/bin/env bash
# Launch Herduck from a Herdr action or pane.
set -euo pipefail

if ! command -v herduck >/dev/null 2>&1; then
  cat <<'MSG'
Herduck is not on PATH yet.

Install (Alpha, from source):
  git clone https://github.com/wenhanweime/herduck.git
  cd herduck && just install

Or try: npx herduck@alpha
(Note: the npm package may lag behind the latest Work features.)

Site: https://www.herduck.com
MSG
  exit 1
fi

# Pane mode: stay in the foreground so the Herdr overlay keeps the TUI.
# Action mode: same — Herdr owns the terminal for action commands that print,
# but for a full TUI we prefer the declared [[panes]] entrypoint.
exec herduck
