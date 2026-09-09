# Herduck

Herduck is an independent terminal workspace manager for AI coding-agent sessions. It combines a
persistent terminal runtime with four navigation tabs:

- **Agents** for live workspaces, tabs, and panes.
- **Sessions** for conversations ordered by recent activity.
- **Projects** grouped by working directory.
- **Topics** grouped by semantic meaning across projects.

Herduck builds and installs its own `herduck` executable. It is derived from Herdr and runs
independently; no separate Herdr installation is required.

## Requirements

- macOS or Linux for the initial source release (Windows is not yet validated)
- Rust (the repository pins the supported toolchain in `rust-toolchain.toml`)
- Zig 0.15.2 for the vendored terminal parser build

On macOS, install the build tools with Homebrew:

```bash
xcode-select --install # if Apple command-line tools are not installed
brew install rustup zig@0.15
rustup-init
export ZIG="$(brew --prefix zig@0.15)/bin/zig"
"$ZIG" version # must report 0.15.2
```

On Linux, install Rust through [rustup](https://rustup.rs/), your distribution's C/C++ build
tools (for example `build-essential` on Debian/Ubuntu), and [Zig 0.15.2](https://ziglang.org/download/).
Place that Zig version on `PATH`, or set `ZIG` to its executable. An unversioned package-manager
install may select an incompatible newer Zig.

## Install from source

```bash
git clone https://github.com/wenhanweime/herduck.git
cd herduck
cargo install --path . --locked
```

On macOS, re-sign the locally built executable after installation or replacement:

```bash
codesign --force --sign - "${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin/herduck"
```

Then run `herduck`. This signing step also applies when upgrading a local macOS build.

`cargo install` places the binary in Cargo's bin directory, normally `~/.cargo/bin`. Ensure that
directory is on `PATH`. This release installs from the complete Git checkout, including its
vendored dependencies. It is not published to crates.io; `cargo install herduck` is not supported.
To upgrade, check out a newer reviewed release and repeat `cargo install --path . --locked`.

For development without installing:

```bash
cargo run --locked -- --help
cargo run --locked
```

## Usage

Run `herduck` to open or attach to the persistent TUI. A server is started automatically; normally
you do not need to run `herduck server` yourself.

The welcome offers **View configuration** or **Skip**. Settings is available from every navigation
tab (Ctrl+B, then S). **Sessions**, **Summaries**, and **Session names** show the loaded configuration
and priorities. Click **Open config file** to edit `config.toml`, save, then reopen Settings to reload.
**Start session** in Sessions uses the configured default Agent; the default is a plain shell.
New tabs and splits open shells, while Resume uses the original Agent.
```bash
herduck
herduck --help
herduck status
herduck server stop
```

Fresh installations use `~/.config/herduck` for configuration and `~/.local/state/herduck` for
runtime state. Debug builds use the `herduck-dev` namespace. Standard XDG directory overrides apply.

Existing ORK3 installations keep their data in place: if the corresponding Herduck configuration
directory does not exist, Herduck reuses `ork3` or `ork3-dev` and the matching state namespace.
The rename does not move or delete configuration, the session catalog, or saved runtime state.
The path shown in Settings is the active configuration file.

The primary environment overrides are:

```text
HERDUCK_CONFIG_PATH
HERDUCK_SOCKET_PATH
HERDUCK_CLIENT_SOCKET_PATH
```

The corresponding `ORK3_*` variables remain accepted for compatibility; an explicit `HERDUCK_*`
value takes precedence. Herduck stays separate from an upstream Herdr installation. See
[Configuration](docs/configuration.md#paths-and-compatibility) for path selection and legacy sockets.

Run `herduck --default-config` for an annotated configuration template and `herduck config check`
to validate your settings. See [Configuration](docs/configuration.md) for title language,
history roots, temporary-runner filters, and personal settings that stay outside the repository.

## Session summaries

Configure API or local Agent sources in `config.toml`. **Settings → Summaries** displays them as a
numbered list, including model order, command or endpoint, and key environment-variable reference.
Herduck tries entries from top to bottom and stops at the first usable result. All changes happen in
the file through **Open config file**; save and reopen Settings to apply them.

**Settings → Session names** uses the Summary order by default, with a local text-based name as the
final fallback. Configure `title_providers` for a separate naming order, or `title_providers = []`
for local-only names. Manual names are retained.

New installations start with generation **Off** (`mode = "pending"`). Set `mode = "auto"` and define
sources to enable ordered fallback. **Offline names only** (`mode = "local"`) creates basic names
without model requests and keeps topics. Reloading these settings requires no server restart;
results from the previous configuration cannot overwrite them. Model-based summaries send selected
conversation content to the chosen provider, including when using a local Agent CLI.
See [Configuration](docs/configuration.md) for Agent/API examples and the complete file workflow.

For an explicit offline choice in your configuration:

```toml
[projects.summary]
mode = "local"
```

To use your own OpenAI-compatible gateway, keep the key in the environment and only reference
its name from `config.toml`:

```toml
[projects.summary]
mode = "auto"

[[projects.summary.providers]]
id = "openrouter"
kind = "openai_compatible"
endpoint = "https://openrouter.ai/api/v1/chat/completions"
api_key_env = "OPENROUTER_API_KEY"
models = ["openrouter/free"]
```

The same provider shape works with LiteLLM, LM Studio, and Ollama (for example,
`http://localhost:11434/v1/chat/completions`). Configure local Agent fallback order under `[[projects.summary.providers]]`. Store API keys
in environment variables and reference them through `api_key_env`; do not commit credentials.

OpenCode Zen paid models are opt-in. Add a provider explicitly and export its key; do not use a
placeholder value such as `public`:

```toml
[[projects.summary.providers]]
id = "opencode_zen"
kind = "openai_compatible"
endpoint = "https://opencode.ai/zen/v1/chat/completions"
api_key_env = "OPENCODE_ZEN_API_KEY"
models = ["your-zen-model"]
```

An optional `opencode_free` provider uses a keyless remote service and sends no `Authorization`
header. It still sends conversation content over the network. Free access is best-effort and
subject to the provider's availability and rate limits.

The complete executable scope, fallback rules, privacy boundaries, and acceptance criteria are in
[docs/PRD-public-summary-providers.md](docs/PRD-public-summary-providers.md).

## Build and test

```bash
just release
just test
```

Install `just`, `zsh`, and Python 3 for the repository recipes. Check public-source portability
and run the full repository gate:

```bash
just check-public
just test-public
just check
```

For a focused local test run, use `just test --bin herduck <test-name-filter>`.

## Contributing and security

See [CONTRIBUTING.md](CONTRIBUTING.md) for development and review, and
[SECURITY.md](SECURITY.md) for private vulnerability reporting and data boundaries.

## License and origin

Herduck is licensed under AGPL-3.0-or-later. It includes modified source originally imported from
Herdr v0.7.4; the original copyright notices and license are preserved. Exact upstream commit and
checksum information is recorded in [docs/UPSTREAM.md](docs/UPSTREAM.md). Herduck is independently
maintained and is not an official Herdr release.
