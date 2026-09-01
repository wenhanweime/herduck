# ORK3

ORK3 is an independent terminal workspace manager for AI coding-agent sessions. It combines a
persistent terminal runtime with three first-class navigation views:

- **Sessions** for the live terminal hierarchy.
- **Projects** grouped by real working directory and ordered by the newest session.
- **Clusters** grouped by semantic topic and ordered by the newest session.

ORK3 builds and installs its own `ork3` executable. It does **not** require Herdr to be installed.

## Requirements

- macOS, Linux, or Windows
- Rust (the repository pins the supported toolchain in `rust-toolchain.toml`)
- Zig 0.15.2 for the vendored terminal parser build

On macOS, install the build tools with Homebrew:

```bash
brew install rustup zig
rustup-init
```

## Install from source

```bash
git clone https://github.com/wenhanweime/ork3.git
cd ork3
cargo install --path . --locked
ork3
```

`cargo install` places the binary in Cargo's bin directory, normally `~/.cargo/bin`. Ensure that
directory is on `PATH`.

For development without installing:

```bash
cargo run --locked -- --help
cargo run --locked
```

## Usage

Run `ork3` to open or attach to the persistent TUI. A server is started automatically; normally
you do not need to run `ork3 server` yourself.

```bash
ork3
ork3 --help
ork3 status
ork3 server stop
```

Production data lives under `~/.config/ork3`. Debug builds use `~/.config/ork3-dev` so development
does not overwrite the installed application state.

The primary environment overrides are:

```text
ORK3_CONFIG_PATH
ORK3_SOCKET_PATH
ORK3_CLIENT_SOCKET_PATH
```

Upstream environment variables are not used for ORK3 configuration or socket selection, so ORK3
cannot accidentally attach to a separately installed upstream runtime.

## Session summaries

ORK3 can create session titles and Cluster topics with no account or API key. The default
`auto` mode tries the OpenCode Zen public/free endpoint on a best-effort basis, then any
configured local Agent CLI, and finally the deterministic local algorithm. A 429, missing Agent,
network error, or malformed response never blocks the TUI.

To force completely offline behavior:

```toml
[projects.summary]
mode = "local" # auto | llm | local
```

To use your own OpenAI-compatible gateway, keep the key in the environment and only reference
its name from `config.toml`:

```toml
[projects.summary]
mode = "llm"

[[projects.summary.providers]]
id = "openrouter"
kind = "openai_compatible"
endpoint = "https://openrouter.ai/api/v1/chat/completions"
api_key_env = "OPENROUTER_API_KEY"
models = ["openrouter/free"]
```

The same provider shape works with LiteLLM, LM Studio, and Ollama (for example,
`http://localhost:11434/v1/chat/completions`). Local Agent presets for OpenCode, Pi, Codex, and
Hermes are included by default; remove or reorder them under `[[projects.summary.providers]]` if
needed. ORK3 never stores or logs the value of an API key.

The complete executable scope, fallback rules, privacy boundaries, and acceptance criteria are in
[docs/PRD-public-summary-providers.md](docs/PRD-public-summary-providers.md).

## Build and test

```bash
cargo build --release --locked
cargo test --locked
```

If `just` and `cargo-nextest` are installed, the full repository gate is:

```bash
just check
```

## License and origin

ORK3 is licensed under AGPL-3.0-or-later. It includes modified source originally imported from
Herdr v0.7.4; the original copyright notices and license are preserved. Exact upstream commit and
checksum information is recorded in [docs/UPSTREAM.md](docs/UPSTREAM.md).
