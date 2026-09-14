<p align="center">
  <img src="assets/herduck-logo.png" alt="Herduck — Make AI work for you. A duck in a white hood, working at a laptop." width="880">
</p>

<h1 align="center">The persistent work layer for agents.</h1>

<p align="center">Agents execute. Herduck keeps the work continuous.</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#features">Features</a> ·
  <a href="#for-people-and-agents">For people and agents</a> ·
  <a href="#where-work-is-going">Product direction</a> ·
  <a href="README.zh-CN.md">简体中文</a>
</p>

Herduck is a **terminal workspace for people and agents to organize and continue work together**.
It combines **tmux-style Agent window management** with related conversations and project context:
run your Agents, find the work they belong to, and return to the right conversation to move it forward.

Built on [Herdr](https://github.com/ogulcancelik/herdr), it keeps the familiar workspaces, tabs,
split panes, and terminals that keep running when you detach. **People use the terminal UI;
Agents use the CLI and JSON API.** The source preview adds saved goals and next steps that both
can update, alongside recent progress and executable follow-ups.

The goal: come back to a piece of work and understand **what we are trying to finish, where it
stands, what needs attention, and what to do next**.

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

**Work is the new name for Topics in GitHub source.** The npm alpha still installs
**0.1.0-alpha.2**, which uses the previous name. Choose the version for the capabilities you need:

| Version | Available capabilities |
| --- | --- |
| **npm alpha · 0.1.0-alpha.2** | Agent windows and activity, a local session library, directory Projects, related conversations under Topics, and CLI/API controls. |
| **Source preview · 0.1.0-alpha.4** | The Work name, saved goals and next steps, recent progress descriptions, and follow-ups sent to the original Agent. [Preview branch and usage](https://github.com/wenhanweime/herduck/pull/1). |

The launcher downloads a native binary and verifies its SHA-256 hash on first use; Rust and Zig
are not needed. Install and sign in to your Agent CLIs separately. Prebuilt downloads support
Apple silicon and Intel Macs, and x64/arm64 Linux with glibc 2.39+ (such as Ubuntu 24.04).
Windows and Alpine/musl are not supported by these downloads.
See [installation](docs/installation.md) for source builds, direct binaries, and upgrades.

## Features

Four views connect the work with its execution, directories, and history:

| View | What it helps you do |
| --- | --- |
| **Agents** | Arrange terminal windows and see which Agents are working or need your attention. |
| **Sessions** | Find supported Agent histories across your device and return to the right conversation. |
| **Projects** | Organize by working directory, create project directories and workspaces, and browse their conversations. |
| **Work** | Bring related conversations together across directories and Agents; the source preview adds shared goals and next steps. |

### Keep execution in view

Run Agent CLIs side by side. Split and resize panes with the mouse, switch tabs and workspaces,
and detach without stopping the terminals. Open `herduck` to attach again.

Activity indicators and configurable notifications show when an Agent is working, waiting for
input, idle, or inactive. Select an Agent to reach its terminal. An idle Agent is ready for more
input; the result still needs to be checked before the work can be called complete.

![Herduck v0.1.0-alpha.2 native Agents view with split terminals and Agent activity](assets/screenshots/agents.png)

*Screenshots show native Ghostty windows running v0.1.0-alpha.2 with prepared example histories.
That version labels Work as Topics. [Capture notes](assets/screenshots/README.md).*

### Bring related conversations together

**Work** groups related conversations by meaning, including conversations from different
directories or Agents. Follow several pieces of work, expand a group, and inspect its history.
Automatic organization is opt-in; existing groups remain available when generation is off or
a model source is unavailable.

![Herduck v0.1.0-alpha.2 related conversations view, labelled Topics in this release](assets/screenshots/topics.png)

**Projects** keeps the familiar directory view, whether the folder holds code, documents, or
other working material. Create a project directory and a workspace for it; supported conversations
from that directory appear in its project group.

![Herduck v0.1.0-alpha.2 native Projects view grouping sessions by working directory](assets/screenshots/projects.png)

### Return to the right conversation

**Sessions** discovers histories from **Claude Code, Codex, Pi, OpenCode, and Grok** on this device.
Additional history directories can be configured. Search the library and preview supported
transcripts without launching an Agent. OpenCode history indexing is supported; its transcript
preview is not yet available.

Continue a supported conversation with its original Agent. Herduck focuses a matching running
session when one exists, or resumes its native history when supported. Work and Projects give
you two more ways to find that same history.

![Herduck v0.1.0-alpha.2 native Sessions view with history and resume controls](assets/screenshots/sessions.png)

### Read recent progress and act on it — source preview

Opening a Work group or Project shows descriptions from recent requests and Agent replies,
alongside suggested next steps. A Work group also keeps an editable goal, up to three next steps,
and a blocker note. Its saved plan survives a server restart.

Choose **Continue with this** to send a selected follow-up to its original Agent. Herduck reuses
a matching live session or resumes the original conversation; busy Agents receive queued input
when ready. **View conversation** opens the supporting context. When an Agent needs your answer,
open its session to respond. Controls and generated descriptions follow the configured Chinese
or English language; the navigation tabs keep their English names.

This preview reads progress and suggestions from individual sessions. Combining evidence into
a work-wide account and assembling context across sessions are planned. Follow-up delivery
records last for the running server's lifetime; “sent” confirms delivery, while the outcome
must still be verified. [Preview usage and API](https://github.com/wenhanweime/herduck/blob/feat/topic-cover-alpha.3/docs/project-overview.md).

## For people and agents

**Agents can manage work, too.** The CLI and JSON API can list Agents, read terminal output,
send input, wait for state changes, and create or manage workspaces, tabs, and panes:

```sh
herduck workspace create --cwd /path/to/project --label my-project
herduck agent list
```

In the **source preview**, Agents can also read progress, update saved goals, next steps and
blockers, and start or check a follow-up. The interface reads the same saved plan. For example,
with the matching server running:

```sh
herduck topic list
herduck topic cover get TOPIC_KEY
herduck topic overview get TOPIC_KEY
herduck topic cover update TOPIC_KEY \
  --next-step "Review the latest result" \
  --blocked-note "Waiting for feedback"
```

These commands return JSON. Replace `TOPIC_KEY` with a key from the list. **Work keeps the existing
`herduck topic` commands and `topic.*` API names for compatibility.** See the
[plan API and guarded updates](https://github.com/wenhanweime/herduck/blob/feat/topic-cover-alpha.3/docs/topic-covers.md)
and [follow-up controls](https://github.com/wenhanweime/herduck/blob/feat/topic-cover-alpha.3/docs/project-overview.md#cli-and-socket-api).

## Where Work is going

Work should outlast the session that started it. We are building toward a shared working context
that a person or a different Agent can pick up and continue:

- **Keep explicit state small:** a goal, lifecycle, blocker, and next steps that people and Agents can read and update.
- **Understand progress from evidence:** connect relevant history to the current work, retain sources, and distinguish confirmed facts from uncertain conclusions.
- **Continue with context:** carry the chosen next step, goal, constraints, and relevant history into execution, then make the result available for the next visit.

This complete Work flow is planned. Today's source preview provides saved plans and session-level
follow-ups as its foundation. Work identity across regrouping, work-wide synthesis, lifecycle
updates, and context assembly are the next steps; cross-device continuity is future work.

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
marking keeps the terminal and process alive. **Automatic reclamation of inactive background
processes, while retaining saved history, is planned.** [Inactive Agent settings](docs/configuration.md#inactive-agents).

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
