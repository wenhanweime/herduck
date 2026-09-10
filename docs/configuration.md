# Configuration

Run `herduck --default-config` to see the annotated template and `herduck config check` to validate
the active file. **Settings → Open config file** opens that file and creates a short template
when it is missing.

Keep personal paths, provider choices, and runner filters in that file. You can run the same
source checkout or binary as other users without carrying a private code branch.

## Paths

Fresh macOS and Linux installations use `~/.config/herduck/config.toml` and the
`~/.local/state/herduck` state directory. Debug builds use `herduck-dev` instead.
`XDG_CONFIG_HOME` and `XDG_STATE_HOME` replace the corresponding base directories.
The configuration directory also holds `projects/catalog.sqlite3`, saved terminal layouts, and
runtime logs. Plugin and agent-detection state use the state directory. `herduck --help` displays
the active log paths; named sessions keep their runtime files under `sessions/<name>/`.

`HERDUCK_CONFIG_PATH` selects a specific configuration file. `HERDUCK_SOCKET_PATH` and
`HERDUCK_CLIENT_SOCKET_PATH` override the API and TUI sockets. The default socket names are
`herduck.sock` and `herduck-client.sock` in the configuration directory; named sessions keep their
sockets under `sessions/<name>/`. The active configuration path is shown in Settings.

## Edit the configuration file

**Settings → Sessions / Summaries / Session names** displays the loaded configuration.
These pages are read only. Click **Open config file** (or press Enter) to edit it in your system
text application. Herduck creates a short commented template if the file is missing; it never
replaces an existing file when opening it, including a file with invalid TOML.

Save the file, close Settings, and reopen Settings to load the changes. You can also use
**Reload Config** (by default Ctrl+B, then Shift+R) or run `herduck server reload-config`.
Invalid changes keep the previous working configuration and show a diagnostic;
`herduck config check` reports the details. Saving in an external editor alone does not trigger a reload.

The path is shown on each page and respects the environment overrides above. macOS uses the default
text editor and Linux uses the file association provided by `xdg-open`. Windows support is unvalidated.
On a machine without a desktop application, open the displayed path with your editor and reload.
On Linux, install a text editor and associate TOML files with it. If the system opens a browser or
offers a download, edit the displayed path directly (for example with `nano`) and reopen Settings.
Herduck uses the desktop file association; it does not install an editor.

`herduck config check` validates the file in a separate process. If it is invalid, a new process
falls back to defaults; a running session instead keeps its last valid settings when reload fails.
The command does not reset a running session's configuration.

Categories stay visible on the left in wide terminals and wrap across the top in narrow ones.
Tab / left / right changes category; up/down, Page Up/Down, Home/End, or the mouse wheel scrolls
through full paths and source details. Appearance previews still use Apply; alerts and display
toggles save when selected.

## First session and default Agent

The welcome offers **View configuration** and **Skip**. Both dismiss the welcome permanently.
View configuration opens the ordinary Sessions settings page; neither action enables summaries
or launches an Agent. A fresh installation keeps generation Off and uses a shell for new sessions.

**Settings → Sessions** shows the default Agent, discovered history directories, and any pending
restart requirement. **Start session** (Ctrl+N) uses the loaded `default_agent`, through the runtime
API. It does not save or change configuration and sends no initial prompt.

```toml
[session]
default_agent = "codex" # "shell" or an empty value opens a plain terminal
```

This setting only controls **Start session** in Settings. New tabs and splits open shells;
resuming a history entry uses its original Agent. Detecting an installed Agent does not change
the default. The value is a single executable, without shell arguments. Its login and model
configuration remain with that Agent. Summary and naming models are configured separately below.

## Summary sources and fallback order

**Settings → Summaries** displays generation mode and numbered sources, with their model order,
Agent command or API URL, and key environment-variable name. Edit the corresponding entries in
`config.toml` to add, remove, or move them. Herduck tries source 1, then 2, and so on, stopping at the
first usable result. Within each source, models are tried in the listed order.

```toml
[projects.summary]
mode = "auto"
title_language = "en"

# Priority 1: use the installed Codex Agent and its default model/login.
[[projects.summary.providers]]
id = "codex"
kind = "cli"
models = []

# Priority 2: try an OpenAI-compatible API.
[[projects.summary.providers]]
id = "summary-api"
kind = "openai_compatible"
endpoint = "https://api.example.com/v1/chat/completions"
api_key_env = "SUMMARY_API_KEY"
models = ["preferred-model", "backup-model"]
```

Use a full chat-completions endpoint. Omit `api_key_env` if no key is required; otherwise export
that variable before starting the Herduck server. Only its name is stored in the file and displayed
in Settings. Supported summary Agents are OpenCode, Pi, Codex, and Hermes. Their own configuration
supplies authentication and provider settings; a `command` field can select the executable path.
Agent requests may use remote services. Model IDs must be understood by that Agent or API.

An empty Agent `models = []` follows its default model; an API needs at least one model ID.
The same Agent may appear more than once with different models. A rejected model advances to the
next model; a startup or transport failure advances to the next source.

Generation modes:

- `mode = "pending"` — **Off**, the default: keep names and topics, without starting generation.
- `mode = "auto"` — **Ordered fallback**: use the configured Summary and naming chains. If all
  sources fail, keep existing topics and derive a basic name from local session text.
- `mode = "local"` — **Offline names only**: generate basic names without model requests; keep topics.

`llm` is also accepted for ordered fallback. Off and Offline keep the source lists for later use.
An explicit `providers = []` disables model attempts. Omit it when using `[[projects.summary.providers]]`
entries: defining the same list in both forms is invalid TOML. Older explicit auto/llm configurations
without a provider list retain their compatibility presets, which Settings shows in their active order.

Reloading applies summary and naming changes without restarting the server. Results from the previous
configuration cannot overwrite names or topics after the new configuration is applied. Existing names,
manual names, topics, and live sessions are retained. Selected conversation content is sent to the
configured provider when model generation runs, including when using a local Agent CLI.

## Session names

**Settings → Session names** shows the loaded language and naming source order. Session names inherit
the Summary order by default. Omit `title_providers` to use that default; set it to `[]` for local-only
names, or add `[[projects.summary.title_providers]]` entries for an independent chain with the same fields
as Summary sources. The final fallback is a name derived from local session text. Manual names are kept.

```toml
[projects.summary]
mode = "auto"
title_language = "zh"
# Optional: local-only names while summaries keep their own model sources.
title_providers = []
```

Titles default to English. Canonical language values are `en` and `zh`; `english`, `chinese`, and
`zh-CN` are accepted aliases. Keep these options inside `[projects.summary]`, before any `[[...]]`
provider entries. The shared `mode` governs both naming and topic generation: independent naming
sources do not bypass Off or Offline. Save and reopen Settings to apply the new configuration.

## Disposable runner directories

Projects group sessions by working directory. If your own automation creates disposable
working directories, configure their directory-name prefixes:

```toml
[projects]
ephemeral_cwd_prefixes = ["ci-worker-", "batch-worker-"]
```

The default list is empty. Each prefix matches a runner directly beneath a system temporary
directory, including its descendants. `/tmp/ci-worker-123/state` matches this example;
`/tmp/my-project/ci-worker-source` does not. Matching is case-sensitive and literal, so `%`, `_`,
and `*` are not wildcards. Empty prefixes and paths containing `/` or `\` are rejected.

Prefix changes require a Herduck restart. Config reload reports that it is keeping the active
catalog settings. On the next start, Herduck applies the new rules to existing automatic assignments
and future scans. Removing a prefix restores directory visibility unless a built-in scratch
directory rule still applies. A missing working directory is grouped as Unclassified.

Filtering changes directory membership only: sessions, semantic topics, titles, and live
associations remain in the catalog. Manually locked assignments keep their chosen project.

Additional agent history roots are configured under `[projects.adapters.<agent>]`. Standard
locations are always retained; empty lists add no extra locations. Custom roots do not restrict
the scan to those directories. The default-config template lists examples for every adapter.
Edit these roots in `config.toml`; **Settings → Sessions** shows the saved directories and warns
when a restart is required. Remove an extra entry from the file when it is no longer needed.

```toml
[projects.adapters.codex]
roots = ["~/other-codex-home/sessions"]
```

## Updates

Automatic version and agent-manifest checks default to `false`. Self-update remains disabled;
upgrade through npm or repeat your native/source installation. Installing a new client does not
replace an already running server. See [upgrades and runtime storage](installation.md#upgrade-storage-and-removal).

## Public-source checks

Run `just check-public` before sharing changes. CI runs the same check on source, tests,
documentation, build configuration, and tooling. It reads tracked files and new nonignored
files, including current edits. Ignoring a file does not exempt it if Git already tracks it.

The check rejects personal home paths (including encoded session paths), private runner names,
and machine-specific provider aliases. Use neutral example paths in tests. Local files under
`.local/`, untracked ignored notes, vendored upstream code, and the checker's policy fixtures
are outside the scan; upstream attribution and compatibility identifiers remain valid.
