<p align="center">
  <img src="assets/herduck-logo.png" alt="Herduck — Make AI work for you. A duck in a white hood, working at a laptop." width="880">
</p>

<h1 align="center">Coding. Marketing. Office work. One AI workspace.</h1>

<p align="center">
  Build the product. Plan the launch. Pick up any conversation where you left off.
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#one-launch-many-conversations">See it in action</a> ·
  <a href="docs/configuration.md">Configuration</a> ·
  <a href="README.zh-CN.md">简体中文</a>
</p>

Use Codex to fix checkout, Claude Code to draft a launch story, and an Agent to turn
meeting notes into next actions. Herduck brings those terminals and conversations together.
Keep using your own Claude Code, Codex, OpenCode, and Pi CLIs.

## Install

With **Node.js 20+** on macOS or Linux:

```sh
npm install -g herduck@alpha
herduck
```

Or try it without a global install:

```sh
npx --yes herduck@alpha
```

This is an **alpha release**. The launcher downloads a verified native binary on first use;
Rust and Zig are not needed. Install and sign in to your Agent CLIs separately.

Prebuilt downloads support Apple silicon and Intel Macs, and x64/arm64 Linux with glibc 2.39+
(such as Ubuntu 24.04). Windows and Alpine/musl are not supported by these downloads.
See [installation](docs/installation.md) for version pinning, GitHub archives, direct binaries,
source builds, upgrades, and troubleshooting.

## One launch, many conversations

Your first-100-users plan touches product, marketing, and operations. **Topics** brings the
related conversations together across folders and Agents: the landing page, Product Hunt
story, launch checklist, and beta invitations are all part of **AI product launch**.

Other example Topics include **Vibe coding to a paid MVP**, **One idea, five channels**,
and **Meetings to next actions**. Open a conversation to see the context and continue it.
Enable model-based organization to generate Topics; existing groups remain available when generation is off.

![Native Herduck Topics view: an AI product launch connects coding, marketing, and office conversations](assets/screenshots/topics.png)

## Ship the MVP and prepare the launch

In **Agents**, review the paid MVP in Codex, keep its local tests visible, and open launch-copy
context in Claude Code. Switch focus and resize panes with the mouse.
Closing the client leaves the server and terminals running; open `herduck` to attach again.

![Native Herduck Agents view with billing context in Codex and launch materials in Claude Code](assets/screenshots/agents.png)

## Keep each workstream in its own project

**Projects** groups conversations by working directory. Return to LaunchDesk for billing and
onboarding, Growth Studio for marketing, or Founder Office for proposals and weekly priorities.
Expand a project and preview the conversation before resuming it.

![Native Herduck Projects view with separate product, marketing, content, and office projects](assets/screenshots/projects.png)

## Find the meeting that had the answer

**Sessions** puts recent conversations from supported local Agents in one list. Find the meeting
follow-up, check its owners and deadlines, then continue with the original Agent when available.
If that session is already running, Herduck focuses its terminal.

![Native Herduck Sessions view showing meeting decisions, owners, and next actions](assets/screenshots/sessions.png)

*Native Ghostty window captures of Herduck v0.1.0-alpha.2. LaunchDesk is a demonstration project;
history and Topic labels are prepared examples. Agent panes show native CLIs, a prepared conversation,
and local test output.
“First 100 users” is a launch target. [Capture notes](assets/screenshots/README.md).*

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
