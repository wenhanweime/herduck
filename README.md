<p align="center">
  <img src="assets/herduck-logo.png" alt="Herduck — Make AI work for you. A duck in a white hood, working at a laptop." width="880">
</p>

<h1 align="center">The persistent work layer for agents.</h1>

<p align="center">Agents execute. Herduck keeps the work continuous.</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#features">Features</a> ·
  <a href="#for-people-and-agents">For people and agents</a> ·
  <a href="README.zh-CN.md">简体中文</a>
</p>

Herduck is a **terminal workspace for people and agents to organize and continue work together**.
Run your Agents, collect related conversations across directories, and see where to continue.

Claude Code investigates a problem. Codex implements a change. Another Agent finds a blocker.
Tomorrow you open a new session. The requests, decisions, and unfinished steps are scattered
across those conversations; you have to piece the work back together.

**Work keeps the goal, blocker, and next steps beside the conversations that support them.**
Open a piece of work to see recent progress, review the evidence, and continue with the relevant Agent.
The plan stays available across sessions and server restarts.

Built on [Herdr](https://github.com/ogulcancelik/herdr), Herduck retains tmux-style workspaces,
tabs, split panes, and terminals that keep running when you detach. Herdr answers “Where are
the Agents?” Herduck adds “What work are they contributing to, and where can I continue?”
People use the terminal UI; Agents use the CLI and JSON API.

## Install

On macOS or Linux, install the [required Rust and Zig toolchain](docs/installation.md#build-from-source), then:

```sh
git clone https://github.com/wenhanweime/herduck.git
cd herduck
just install
herduck
```

Install and sign in to your Agent CLIs separately. Herduck uses their existing tools and accounts.
See [installation](docs/installation.md) for system dependencies, macOS signing, PATH setup,
prebuilt packages, and upgrades.

## Features

Four views connect work with execution, directories, and history.

| View | What it helps you do |
| --- | --- |
| **Agents** | Arrange terminal windows and see which Agents are working or need your attention. |
| **Sessions** | Find supported Agent histories across your device and return to the right conversation. |
| **Projects** | Organize by working directory, create project directories and workspaces, and browse their conversations. |
| **Work** | Bring related conversations together across directories and Agents, with shared goals and next steps. |

### Keep execution in view

Run Agent CLIs side by side. Split and resize panes with the mouse, switch tabs and workspaces,
and detach without stopping the terminals. Open `herduck` to attach again.

Activity indicators and configurable notifications show when an Agent is working, waiting for
input, idle, or inactive. Select an Agent to reach its terminal. An idle Agent is ready for more
input; the result still needs to be checked before the work can be called complete.

### Bring related conversations together

**Work** groups related conversations by meaning, including conversations from different
directories or Agents. Follow several pieces of work, expand a group, and inspect its history.
Automatic organization is opt-in; existing groups remain available when generation is off or
a model source is unavailable.

**Projects** keeps the familiar directory view, whether the folder holds code, documents, or
other working material. Create a project directory and a workspace for it; supported conversations
from that directory appear in its project group.

### Return to the right conversation

**Sessions** discovers histories from **Claude Code, Codex, Pi, OpenCode, and Grok** on this device.
Additional history directories can be configured. Search the library and preview supported
transcripts without launching an Agent. OpenCode history indexing is supported; its transcript
preview is not yet available.

Continue a supported conversation with its original Agent. Herduck focuses a matching running
session when one exists, or resumes its native history when supported. Work and Projects give
you two more ways to find that same history.

### Read recent progress and act on it

Opening a Work group or Project shows descriptions from recent requests and Agent replies,
alongside suggested next steps. A Work group also keeps an editable goal, up to three next steps,
and a blocker note. Its saved plan survives a server restart.

Choose **Continue with this** to send a selected follow-up to its original Agent. Herduck reuses
a matching live session or resumes the original conversation; busy Agents receive queued input
when ready. **View conversation** opens the supporting context. When an Agent needs your answer,
open its session to respond. Controls and generated descriptions follow the configured Chinese
or English language; the navigation tabs keep their English names.

Progress and suggestions come from recent individual conversations. Follow-up delivery records last
for the running server's lifetime; “sent” confirms delivery, while the outcome still needs verification.
[Usage and API](docs/project-overview.md).

## For people and agents

**Agents can manage work, too.** The CLI and JSON API can list Agents, read terminal output,
send input, wait for state changes, and create or manage workspaces, tabs, and panes:

```sh
herduck workspace create --cwd /path/to/project --label my-project
herduck agent list
```

Agents can also read progress, update saved goals, next steps and
blockers, and start or check a follow-up. The interface reads the same saved plan. For example,
with the matching server running:

```sh
herduck work list
herduck work plan get WORK_KEY
herduck work overview get WORK_KEY
herduck work plan update WORK_KEY \
  --goal "Verify the npm installer on a clean machine" \
  --next-step "Review the latest result" \
  --blocked-note "Waiting for feedback"
```

These commands return JSON. Copy a `canonical_key` from the list into `WORK_KEY`.
Read the plan and overview before choosing a next action. See the
[plan API and guarded updates](docs/topic-covers.md) and [follow-up controls](docs/project-overview.md#cli-and-socket-api).

## Where Herduck fits

The runtime executes Agents; Sessions preserve their conversations; Projects group them by directory.
Work connects related conversations across those boundaries and keeps a shared plan beside the evidence. It complements the memory and knowledge tools your Agents already
use. The current overview reads recent session evidence; it does not reconstruct all past decisions.

## Use your Agents and choose your model sources

Herduck includes screen-state detection for **19 Agent CLIs**, including Claude Code, Codex,
OpenCode, Pi, Gemini CLI, Cursor, GitHub Copilot, Kimi, Grok, and Hermes. Detection, history
indexing, transcript reading, and resume have different coverage; history support is described above.

For work organization and session naming, configure **OpenCode, Pi, Codex, or Hermes CLI sources**,
or an **OpenAI-compatible API**. Sources and their models are tried in your chosen order. Model
rejection advances to the next model; startup or transport failure advances to the next source.
If all sources fail, existing Work groups remain available and session names fall back to local text.
Manual names are preserved. [Configure sources and fallback](docs/configuration.md#summary-sources-and-fallback-order).

### Local data and opt-in generation

Conversation indexing and saved plans use local storage. Model-based summaries, names, and work
organization are **off by default**. When enabled, selected conversation content is sent to your
configured Agent or API sources. Using an Agent CLI as a source can still call that Agent's
remote model. Agent authentication stays with each Agent.

Open **herduck · menu → settings**, or press **Ctrl+B, then S**. The Sessions, Summaries, and
Session names pages show the loaded settings. Choose **Open config file**, edit and save, then
close and reopen Settings to reload. Run `herduck config check` to validate the file.
See [configuration and data locations](docs/configuration.md).

### Keep history available as resource use changes

Browsing history does not launch an Agent. Continuing a conversation starts its Agent when
needed and reuses a matching running session. You can end an unneeded process while retaining
the history its CLI saved for later inspection or supported resume.

An idle Agent is marked **inactive** after a configurable period without activity, one hour by
default. Working Agents and Agents awaiting a response or approval are excluded. Inactivity
marking keeps the terminal and process alive. End unneeded processes when you want to free resources.
[Inactive Agent settings](docs/configuration.md#inactive-agents).

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
