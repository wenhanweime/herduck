<p align="center">
  <img src="assets/herduck-logo.png" alt="Herduck — Make AI work for you. A duck in a white hood, working at a laptop." width="880">
</p>

<h1 align="center">Sessions end. Work continues.</h1>

<p align="center">Agent window management. Shared project context for people and Agents.</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#features">Features</a> ·
  <a href="#toward-persistent-work-state">Product direction</a> ·
  <a href="docs/configuration.md">Configuration</a> ·
  <a href="README.zh-CN.md">简体中文</a>
</p>

Herduck brings **Agent terminals, conversation history, and project views** into one terminal
workspace. Run several Agents, see which ones need attention, find their related conversations,
and pick up the right context to continue.

It builds on [Herdr](https://github.com/ogulcancelik/herdr), a terminal manager and runtime for
Agents with a workflow familiar to **tmux** users: workspaces, tabs, split panes, and terminal
sessions that keep running when you detach. Herduck adds organization across Agents and sessions,
with a terminal UI for people and a CLI and JSON API for Agents.

Our goal is to make returning to work straightforward: **what are we trying to finish, where
does it stand, what needs a decision, and what should happen next?** We are building toward a
persistent work state that both people and Agents can read, maintain, and act on.

| Availability | What you can use |
| --- | --- |
| **npm alpha · 0.1.0-alpha.2** | Agent windows and activity, a local session library, directory Projects, semantic Topics, and CLI/API controls. |
| **Source preview · 0.1.0-alpha.4** | Saved Topic plans, descriptions of recent progress, and follow-ups sent to the original Agent. See [the preview PR](https://github.com/wenhanweime/herduck/pull/1). |
| **Planned** | Persistent work-level synthesis, decisions and failed attempts, prioritized actions with dependencies, and results written back into work state. |

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

The npm alpha currently installs **0.1.0-alpha.2**. The launcher downloads a native binary and
verifies its SHA-256 hash on first use; Rust and Zig are not needed. Install and sign in to your
Agent CLIs separately. Source-preview features require the [preview branch](https://github.com/wenhanweime/herduck/pull/1).

Prebuilt downloads support Apple silicon and Intel Macs, and x64/arm64 Linux with glibc 2.39+
(such as Ubuntu 24.04). Windows and Alpine/musl are not supported by these downloads.
See [installation](docs/installation.md) for version pinning, direct binaries, source builds,
upgrades, and troubleshooting.

## Features

Four views connect the execution environment with its history and project context:

| View | What it helps you do |
| --- | --- |
| **Agents** | Manage terminal windows and see Agent activity across Herduck workspaces. |
| **Sessions** | Find supported local Agent histories, including work started outside Herduck, and continue supported conversations. |
| **Projects** | Follow work by directory; create project directories and workspaces, and browse each project's conversations. |
| **Topics** | Automatically bring related conversations together across directories and Agents when semantic organization is enabled. |

### Keep running Agents in view

Run Agent CLIs side by side. Split and resize panes with the mouse, switch tabs and workspaces,
and detach without stopping the terminals. Open `herduck` to attach again.

Activity indicators and configurable notifications help you notice an Agent working, waiting
for input, or becoming idle or inactive. Select an Agent to reach its terminal. Runtime activity
describes the Agent; confirming that the work is complete still requires its result.

![Native Herduck Agents view showing split terminal panes and the Agent activity overview](assets/screenshots/agents.png)

### Find related work across sessions

**Topics** groups conversations by meaning, including conversations from different directories
or Agents. Follow several topics, expand a group, and inspect its history. Generation is opt-in;
existing Topics remain available when generation is off or a model source is unavailable.

![Native Herduck Topics view grouping related conversations across directories and Agents](assets/screenshots/topics.png)

**Projects** keeps the familiar directory view. Create a project directory and a workspace for
it; supported conversations from that directory appear in its project group. Each project has
its own view of recent and open sessions.

![Native Herduck Projects view grouping sessions by working directory](assets/screenshots/projects.png)

### Return to the right conversation

**Sessions** discovers histories from **Claude Code, Codex, Pi, OpenCode, and Grok** on this device.
Additional history directories can be configured. Search the library and preview supported
transcripts without launching an Agent. OpenCode history indexing is supported; its transcript
preview is not yet available.

Continue a supported conversation with its original Agent. Herduck focuses a matching running
session when one exists, or resumes its native history when supported. Projects and Topics are
additional ways to find that same history.

![Native Herduck Sessions view showing a conversation list, saved context, and resume controls](assets/screenshots/sessions.png)

*These are native Ghostty window captures of Herduck v0.1.0-alpha.2 with prepared example histories
and Topics. Agent panes show native CLIs, a prepared conversation, and local test output.
They illustrate the available interface. [Capture notes](assets/screenshots/README.md).*

### See recent progress and act on a follow-up — source preview

In **0.1.0-alpha.4**, opening a Topic or Project shows descriptions from recent requests and Agent
replies, along with suggested next steps. A Topic also keeps an editable goal, up to three next
steps, and a blocker note. Its saved plan survives a server restart.

Choose **Continue with this** to send a selected follow-up to its original Agent. Herduck reuses
a matching live session or resumes the original conversation; busy Agents receive queued input
when ready. **View conversation** opens the evidence. If the Agent needs your answer, open it to
respond. Controls and generated descriptions follow the configured Chinese or English language;
the four navigation tabs keep their English names.

This preview derives progress and suggestions from individual sessions. Cross-session work
synthesis is planned. Follow-up delivery records currently last for the running server's lifetime;
“sent” confirms delivery, while the outcome must still be verified.
See [preview usage and API](https://github.com/wenhanweime/herduck/blob/041f35fc68f8a055f0fcfe427073496433112639/docs/project-overview.md).

### Give people and Agents access to the same workspace

The CLI and JSON API can list Agents, read terminal output, send input, wait for state changes,
and create or manage workspaces, tabs, and panes. For example:

```sh
herduck workspace create --cwd /path/to/project --label my-project
herduck agent list
```

The source preview also exposes Topic-plan updates, project overviews, and follow-up controls
through the public API. Run `herduck agent --help` or see the [API schema](docs/api/herduck-api.schema.json)
for the checked-out version; the preview contract is linked above.

## Toward persistent work state

Work often spans several sessions and Agents. The next step for Herduck is to maintain a shared
account of that work, with each conclusion connected to its evidence:

| Part of the work | What people and Agents should be able to understand |
| --- | --- |
| **Goal and completion criteria** | What are we trying to achieve, and how will we know it is done? |
| **Current progress and blockers** | What is confirmed, what remains uncertain, and what needs attention? |
| **Decisions and attempts** | Why did we choose this approach, and which dead ends should we avoid repeating? |
| **Next actions** | What should happen next, why now, and does it need a person or an Agent? |
| **Results and recent changes** | What changed since the last visit, and which files, outputs, or conversations support it? |

The intended loop is **observe → maintain work state → act → record the result**. Facts,
decisions, and hypotheses must stay distinguishable as context is summarized and passed on.
A new Agent should be able to take over with the goal, constraints, prior attempts, and next
step already available. This work-level memory and action loop is the product direction;
it is not yet implemented in full. Cross-device continuity is also future work.

## Use your Agents and choose your model sources

Herduck includes screen-state detection for **19 Agent CLIs**, including Claude Code, Codex,
OpenCode, Pi, Gemini CLI, Cursor, GitHub Copilot, Kimi, Grok, and Hermes. Detection, history
indexing, transcript reading, and resume have different coverage; history support is described above.

For topic organization and session naming, configure **OpenCode, Pi, Codex, or Hermes CLI sources**,
or an **OpenAI-compatible API**. Sources and their models are tried in your chosen order. Model
rejection advances to the next model; startup or transport failure advances to the next source.
If all sources fail, existing Topics remain available and session names fall back to local text.
Manual names are preserved. [Configure sources and fallback](docs/configuration.md#summary-sources-and-fallback-order).

### Local data and opt-in generation

Conversation indexing and saved plans use local storage. Model-based summaries, names, and Topics
are **off by default**. When enabled, selected conversation content is sent to your configured
Agent or API sources. Using an Agent CLI as a source can still call that Agent's remote model.
Agent authentication stays with each Agent.

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
