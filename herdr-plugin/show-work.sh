#!/usr/bin/env bash
# Print a short Herduck work snapshot for the current Herdr context.
set -euo pipefail

if ! command -v herduck >/dev/null 2>&1; then
  cat <<'MSG'
Herduck is not on PATH yet, so there is no work status to show.

Install: https://www.herduck.com
Repo:    https://github.com/wenhanweime/herduck
MSG
  exit 1
fi

cwd="${HERDR_PLUGIN_CONTEXT_JSON:-}"
# Best-effort: if jq exists, pull cwd / workspace label from context for matching later.
hint=""
if command -v jq >/dev/null 2>&1 && [[ -n "$cwd" ]]; then
  hint=$(printf '%s' "$cwd" | jq -r '
    .workspace.cwd // .worktree.path // .pane.cwd // .cwd // empty
  ' 2>/dev/null || true)
fi

echo "=== Herduck work snapshot ==="
if [[ -n "$hint" ]]; then
  echo "Herdr context path: $hint"
  echo
fi

# List works as JSON; keep it readable for a quick pane glance.
if ! list_json=$(herduck work list 2>/dev/null); then
  cat <<'MSG'
Could not talk to Herduck (is the Herduck server running?).

Open Herduck once with the "Open Herduck" action, then try again.
MSG
  exit 1
fi

if command -v jq >/dev/null 2>&1; then
  count=$(printf '%s' "$list_json" | jq 'if type=="array" then length else (.works // .items // []) | length end' 2>/dev/null || echo 0)
  echo "Works visible: $count"
  echo
  # Print a compact table if the shape is an array of objects with common fields.
  printf '%s' "$list_json" | jq -r '
    (if type=="array" then . else (.works // .items // []) end)
    | .[:8][]
    | [
        (.canonical_key // .key // .id // "?"),
        (.goal // .title // .name // ""),
        (.status // .state // ""),
        (.blocker // ""),
        (.next // .next_step // "")
      ]
    | @tsv
  ' 2>/dev/null | awk -F'\t' 'BEGIN{
      printf "%-22s | %-28s | %-12s | %-20s | %s\n","key","goal","status","blocker","next";
      printf "%s\n","----------------------+------------------------------+--------------+----------------------+-----";
    }
    {
      printf "%-22.22s | %-28.28s | %-12.12s | %-20.20s | %s\n",$1,$2,$3,$4,$5
    }'
  echo
  echo "(Showing up to 8. Use Herduck itself for the full Work view.)"
else
  echo "$list_json"
  echo
  echo "(Install jq for a prettier table.)"
fi

echo
echo "Herduck complements Herdr — it keeps goal / status / blocker / next across sessions."
echo "https://www.herduck.com"
