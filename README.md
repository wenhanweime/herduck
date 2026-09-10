<p align="center">
  <img src="assets/herduck-logo.png" alt="Herduck — Make AI work for you. A duck in a white hood, working at a laptop." width="880">
</p>

<h1 align="center">Your AI coding agents. One terminal.</h1>

<p align="center">
  Run agents side by side. Find past conversations. Pick up where you left off.
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#see-all-your-work">See it in action</a> ·
  <a href="docs/configuration.md">Configuration</a> ·
  <a href="README.zh-CN.md">简体中文</a>
</p>

Herduck brings your coding-agent terminals and conversation history into one workspace.
Keep using the CLIs you know, including Claude Code, Codex, OpenCode, and Pi.
Herduck helps you see what is running, find the right conversation, and continue it.

## Install

With **Node.js 20+** on macOS or Linux:

```sh
npm install -g https://github.com/wenhanweime/herduck/releases/download/v0.1.0-alpha.2/herduck-0.1.0-alpha.2.tgz
herduck
```

Or try it without a global install:

```sh
npx --yes --package=https://github.com/wenhanweime/herduck/releases/download/v0.1.0-alpha.2/herduck-0.1.0-alpha.2.tgz herduck
```

Install this release directly from GitHub; the npm registry package is not published yet.
This is an **alpha release**. The launcher downloads a verified native binary on first use;
Rust and Zig are not needed. Install and sign in to your Agent CLIs separately.

Prebuilt downloads support Apple silicon and Intel Macs, and x64/arm64 Linux with glibc 2.39+
(such as Ubuntu 24.04). Windows and Alpine/musl are not supported by these downloads.
See [installation](docs/installation.md) for GitHub-only npm installation, direct binaries,
source builds, upgrades, and troubleshooting.

## See all your work

Keep an Agent building while another reviews. Split panes, resize them with the mouse,
and switch between projects without losing your place. Closing the client leaves the
server and its terminals running; open `herduck` to attach again.

![Two demonstration Agent terminals side by side in Herduck](assets/screenshots/agents.png)

## Find the conversation you meant

**Sessions** brings supported local Agent histories together, with the most recent work first.
Preview a conversation, then resume it with its original Agent when available. Already running?
Herduck focuses that terminal.

![Herduck Sessions showing recent demonstration conversations](assets/screenshots/sessions.png)

## Come back to the right context

**Projects** groups sessions by working directory. **Topics** groups related work across projects
when you enable model-based organization. Your names and existing groups remain available when
generation is off.

![Herduck Projects grouping demonstration conversations by folder](assets/screenshots/projects.png)

*These are captures of the running terminal UI with demonstration conversations and Agent output.*

## Start with your own tools

The welcome opens configuration or lets you skip straight to a shell. Choose **herduck · menu → settings**,
or press **Ctrl+B, then S** from a terminal. The **Sessions**, **Summaries**, and **Session names**
pages show the loaded configuration and source order.

Use **Open config file** to edit, save, then close and reopen Settings to reload.
Run `herduck config check` to check the file. Your Agent authentication stays with each Agent.

Model-based summaries, names, and topics are **off by default**. To enable them, choose your
Agent or API sources in the configuration file. Selected conversation content is sent to those
sources when generation runs. Local-only naming is also available. See
[configuration and privacy](docs/configuration.md) for examples and data locations.

## Development

Follow the pinned [Rust and Zig setup](docs/installation.md#build-from-source), then run:

```sh
just build
just check
```

`just check` runs formatting, public-source checks, npm installer tests, Clippy, and Rust tests.
Repository recipes require `just`, `zsh`, Python 3, and Node.js 20+.
See [CONTRIBUTING.md](CONTRIBUTING.md) for review and [SECURITY.md](SECURITY.md) for private reports.

## License and origin

Herduck is an independent project derived from **Herdr v0.7.4**, distributed under
**AGPL-3.0-or-later**. It installs its own executable and needs no separate Herdr installation.
Original notices are preserved. See [LICENSE](LICENSE) and [upstream provenance](docs/UPSTREAM.md).
