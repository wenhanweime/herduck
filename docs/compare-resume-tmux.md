# Herduck vs `claude --resume`, tmux, Herdr, and other agent session managers

[简体中文](compare-resume-tmux.zh-CN.md)

*Written by the Herduck author. Herduck is an early-stage alpha, so this page tries to say plainly
what each tool is good at and where Herduck is not the right choice. Facts about other tools are
taken from their own documentation (linked at the bottom) as of 2026-10-28 (the section on other session managers: 2026-10-30); check there if
something looks stale.*

## The short version

| If your problem is… | Reach for |
| --- | --- |
| "Re-open yesterday's Claude Code conversation" | `claude --resume` / `claude --continue` — built in, nothing to install |
| "Keep my terminals alive when I close the window or lose SSH" | tmux, or a runtime such as Herdr |
| "See which of my Agents is working, blocked, or idle" | Herdr (or Herduck, which inherits that part) |
| "I run Claude Code, Codex, and others across several folders and no longer remember what each piece of work was for, where it stands, and what is next" | This is the gap Herduck is built for |

These are different layers. Most people can use more than one of them.

## `claude --resume` / `claude --continue`

What it does well:

- `claude --continue` loads the most recent conversation in the current directory;
  `claude --resume` opens a picker or takes a session ID or name (`claude -n "my-work"`, then
  `claude --resume my-work`).
- It restores that conversation's full history, and it is first-party: no extra install, no extra
  moving parts.
- Recent Claude Code versions also have background sessions (`claude agents`, `claude attach`).

Where it stops:

- It is scoped to Claude Code. Your Codex, OpenCode, Pi, or Grok conversations have their own
  resume mechanisms.
- It resumes a *conversation*. It does not record, across conversations, "the goal of this piece
  of work, what is blocking it, and the next step". That part lives in your head or in notes.
- With many sessions across many folders, finding the right one is a search problem.

**Use `claude --resume` when** you work mostly in one Agent and one folder at a time. It is the
simplest thing that works. Herduck does not replace it; its Continue action resumes the original
conversation with the original Agent.

## tmux and other terminal session managers

What they do well:

- Keep terminals and the processes inside them alive after you detach, close the terminal, or
  drop an SSH connection. Mature, scriptable, available almost everywhere.
- Agent-agnostic: tmux does not care what runs inside a pane.

Where they stop:

- They manage terminals, not work. A tmux session named `auth` tells you nothing about the goal,
  the blocker, or the next step unless you maintain that yourself.
- tmux does not know what an Agent is doing. Whether an Agent in a pane is working, waiting for
  your approval, or idle is something you check by looking.
- A reboot ends the processes in the panes. (Plugins such as tmux-resurrect can restore the
  layout; they do not bring back a running Agent's in-memory state.)

**Use tmux when** you want a general-purpose, minimal, battle-tested way to keep terminals alive,
and you are happy keeping the work-level context yourself. Herduck can sit alongside it: Herduck
reads the Agent histories on disk, regardless of how you launched the Agent.

## Herdr

[Herdr](https://herdr.dev) is the project Herduck was derived from, and it is the more mature one
(a much larger user base, regular releases, a plugin marketplace). Per its README, it is "the
runtime your coding agents live on": a background server keeps terminals running when you
detach or lose SSH, every pane is marked working / blocked / idle, agents can drive it through a
CLI and socket API, and it supports multiple machines in one window. It does not wrap or replace
the Agents you run.

Herdr answers **"where are my Agents, and which one needs me?"** Herduck starts from the same
runtime base and adds a layer above it, aimed at **"what work are they contributing to, and
where can I continue?"**

Be aware of the relationship:

- Herduck is an independent project derived from Herdr v0.7.4 (see [UPSTREAM.md](UPSTREAM.md)).
  It ships its own executable and does not need Herdr installed. It is not an official Herdr
  release and not a Herdr plugin.
- Herdr has moved on since v0.7.4. If you only need the runtime — detach/reattach, Agent
  state, multi-machine — Herdr itself is the better-maintained choice and has more features
  there today.

**Use Herdr when** a good runtime is all you need.

## What Herduck adds, and what it costs

What it adds on top of a runtime:

- **Work**: sessions from different Agents and folders gathered into one piece of work, with an
  editable goal, up to three next steps, and a blocker note that survive sessions ending and
  restarts. Your own edits are never overwritten by automatic updates.
- **Sessions**: one searchable list of local conversation histories — Claude Code, Codex, Pi,
  OpenCode, and Grok — including ones that never started inside Herduck.
- **Continue with this**: sends a follow-up to the original Agent, reusing a matching live
  session or resuming the original conversation.
- A CLI and JSON API (`herduck work list`, `herduck work plan get/update`) so an Agent can read
  the same plan a person sees.

What it costs, honestly:

- It is an **alpha**, macOS and Linux only. The Work plans and progress overview described here
  need a build from source (Rust and Zig toolchain); the published npm package
  (`herduck@alpha`) is older and has fewer features (see [installation](installation.md)).
  Expect rough edges.
- Automatic Work grouping, session naming, and progress summaries are **off by default**. They
  need a model source you configure (OpenCode, Pi, Codex, Hermes CLI, or an OpenAI-compatible
  API), and when enabled, selected conversation content is sent to that source. Without them you
  still get the session list and the plan you write yourself.
- OpenCode histories are indexed, but transcript preview is not available yet.
- It is a new project with a very small user base (8 GitHub stars when this page was written).
  Don't assume it has been tested on your setup.
- Its Work layer is only as good as the goal/blocker/next-step notes and the grouping; it does
  not make an Agent smarter or give it a bigger context window.

## Other multi-agent session managers

Several other open-source tools manage many coding-agent sessions in the terminal. The
descriptions below are short summaries of each project's own README, checked on 2026-10-30;
read their pages for the details. Most of them focus on **running** many Agents in parallel
(isolated worktrees, status, notifications). Herduck's focus is different: remembering **what
each piece of work is for and what comes next**, across Agents and days. If parallel execution
is your main problem, one of these may suit you better.

| Tool | What it is, per its README | Worth knowing |
| --- | --- | --- |
| [Claude Squad](https://github.com/smtg-ai/claude-squad) | Terminal app to manage multiple AI terminal agents (Claude Code, Codex, OpenCode, Amp) | Each task gets its own isolated git workspace; tasks can run in the background; review changes before applying. Needs tmux and `gh`. |
| [Agent Deck](https://github.com/asheshgoplani/agent-deck) | Terminal session manager for AI coding agents: one TUI for Claude, Gemini, OpenCode, Codex and more | Shows every session's state in one screen; built-in MCP and skills managers; session groups. Uses tmux. macOS, Linux, WSL. |
| [CCManager](https://github.com/kbwo/ccmanager) | CLI for running many AI coding assistant sessions (Claude Code, Gemini CLI, Codex CLI, Cursor Agent, Copilot CLI, OpenCode and others) across git worktrees and projects | No tmux needed; busy / waiting / idle status; restores sessions after restart. Installs with npm. |
| [cmux](https://github.com/manaflow-ai/cmux) | Ghostty-based macOS terminal with vertical tabs and notifications for AI coding agents | A full terminal app rather than a layer on top; macOS only. |
| [Rove](https://github.com/Sma1lboy/rove) | "The agent multiplexer for your terminal" | Parallel tasks in isolated git worktrees and branches; sessions persist on disconnect; scriptable. |

Claude Code itself now also has **agent view** (`claude agents`) for watching background Claude
Code sessions in one screen — see the [Claude Code section](#claude---resume--claude---continue)
above and [How to keep track of many Claude Code sessions](manage-many-claude-code-sessions.md).

## Quick decision guide

- One Agent, one folder, occasional resume → `claude --resume`.
- Need terminals to survive detach/SSH, nothing more → tmux.
- Want Agent-aware status and a runtime built for coding Agents → Herdr.
- Mainly want many Agents running in parallel, each in its own worktree → see
  [other multi-agent session managers](#other-multi-agent-session-managers).
- Many Agents and folders, work that spans days, and you keep losing the thread of *what the
  work is* → try Herduck, ideally on a non-critical project first, and tell me what breaks:
  [open an issue](https://github.com/wenhanweime/herduck/issues).

## Sources

- Claude Code CLI reference (`--continue`, `--resume`, `--name`, `claude agents`, `claude attach`):
  <https://code.claude.com/docs/en/cli-reference>
- Herdr README and docs: <https://github.com/herdrdev/herdr> · <https://herdr.dev/docs/>
- tmux: <https://github.com/tmux/tmux/wiki> · tmux-resurrect:
  <https://github.com/tmux-plugins/tmux-resurrect>
- Herduck: [README](../README.md) · [UPSTREAM.md](UPSTREAM.md) ·
  [configuration](configuration.md) · [project overview](project-overview.md)
- Other tools (READMEs, checked 2026-10-30): [Claude Squad](https://github.com/smtg-ai/claude-squad) ·
  [Agent Deck](https://github.com/asheshgoplani/agent-deck) · [CCManager](https://github.com/kbwo/ccmanager) ·
  [cmux](https://github.com/manaflow-ai/cmux) · [Rove](https://github.com/Sma1lboy/rove)
- More: [How to keep track of many Claude Code sessions](manage-many-claude-code-sessions.md) ·
  [Herduck and Herdr](herduck-and-herdr.md)
