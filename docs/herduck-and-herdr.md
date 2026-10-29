# Herduck and Herdr: what each one does, and how they work together

[简体中文](herduck-and-herdr.zh-CN.md)

*Written by the Herduck author. Facts about Herdr come from its own README and docs (linked at the
bottom), checked on 2026-10-30. If they have changed, trust Herdr's docs over this page.*

## Short version

- **Herdr** is a terminal runtime for coding agents. It answers: *where are my Agents, and which
  one needs me?*
- **Herduck** started from Herdr's code and adds a layer on top. It answers: *what is this work
  for, where does it stand, and what happens next?*
- They are **separate programs**. Herduck is not a Herdr plugin, and it does not need Herdr
  installed. There is a small optional bridge plugin if you use both.

## What Herdr does

Per its README, Herdr is "the runtime your coding agents live on". In everyday terms:

- A background server keeps your terminals running when you close the window or lose SSH;
  you reattach later.
- Every pane is marked as working, blocked, or idle, so you can see which Agent needs you.
- Agents can drive it themselves through a CLI and socket API.
- It can show several machines in one window, and it has a plugin marketplace.
- It does not wrap or replace the Agents you run — Claude Code, Codex and the rest stay as they are.

Herdr is the older, more mature project, with a much larger user base and regular releases.

## What Herduck adds

Herduck is an independent project derived from **Herdr v0.7.4** (see [UPSTREAM.md](UPSTREAM.md)).
It keeps the runtime part — panes, tabs, workspaces, detach/reattach, Agent states for 19 Agent
CLIs — and adds:

- **Work**: conversations from different Agents and folders gathered into one piece of work, with
  an editable goal, up to three next steps, and a blocker note. These survive sessions ending and
  the server restarting; automatic updates never overwrite what you wrote.
- **Sessions**: one searchable list of local conversation histories from Claude Code, Codex, Pi,
  OpenCode, and Grok, including ones started outside Herduck. (OpenCode histories are indexed,
  but their preview is not available yet.)
- **Continue with this**: sends a chosen next step to the original Agent, reusing its live
  session or resuming the original conversation.
- A CLI and JSON API (`herduck work list`, `herduck work plan get/update`,
  `herduck work overview get`) so an Agent can read the same plan a person sees.

Automatic Work grouping, session names, and progress descriptions are off by default and need a
model source you configure; see [configuration](configuration.md).

## The relationship, plainly

- Herduck ships its own executable (`herduck`) and its own server. Installing Herduck does not
  install or change Herdr.
- It is **not** an official Herdr release. Herdr has moved on since v0.7.4, so for the runtime
  part alone (detach/reattach, Agent state, multiple machines), Herdr itself has more features
  today.
- License: AGPL-3.0-or-later, same as Herdr. Original copyright notices are kept. The commercial
  license offer in Herdr's LICENSE belongs to Herdr's rights holder, not to Herduck.

## Using both: the bridge plugin

If Herdr is your daily terminal and you also want Herduck's Work view, install the thin bridge
from this repository:

```sh
herdr plugin install wenhanweime/herduck/herdr-plugin
```

It needs Herdr 0.7.0 or newer and the `herduck` binary on your `PATH`. It adds two actions:

| Action | What it does |
| --- | --- |
| Open Herduck | Opens Herduck in an overlay pane inside Herdr. |
| Show Herduck work status | Prints up to eight Works from the local Herduck server. Uses `jq` for a neater table if it is installed. |

An optional keybinding (`prefix+h` to open Herduck) is shown in the
[bridge plugin README](../herdr-plugin/README.md).

The plugin only runs the two small shell scripts in [herdr-plugin/](../herdr-plugin/). It does
not move your Herdr sessions into Herduck.

## Which one should you use?

- **Herdr only**: you want a solid terminal runtime that shows which Agent needs you, and you
  remember what each piece of work is about without help.
- **Herduck only**: you mainly want the Work and Sessions views, and you are fine with an
  early alpha (macOS and Linux; Work features need a [build from source](installation.md)).
- **Both, with the bridge**: Herdr stays your main runtime; you open Herduck when you need to
  see the goal, blocker, and next step of a piece of work.

See also: [comparison with `claude --resume`, tmux, Herdr and other tools](compare-resume-tmux.md).

## Sources

- Herdr: <https://github.com/herdrdev/herdr> · <https://herdr.dev/docs/> ·
  [plugin marketplace](https://herdr.dev/plugins/)
- Herduck: [README](../README.md) · [UPSTREAM.md](UPSTREAM.md) ·
  [bridge plugin](../herdr-plugin/README.md)
