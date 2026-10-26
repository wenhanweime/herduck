# Herduck bridge (Herdr plugin)

A **thin bridge** so Herduck shows up in the [Herdr plugin marketplace](https://herdr.dev/plugins/). Herduck itself stays a separate app (a work-memory layer that complements Herdr). This folder does not turn Herduck into a plugin.

## Install

```bash
herdr plugin install wenhanweime/herduck/herdr-plugin
```

You still need the `herduck` binary on your `PATH` (see https://www.herduck.com). The plugin only opens Herduck or prints a short work snapshot.

## Actions

| Action | What it does |
|--------|----------------|
| Open Herduck | Opens the Herduck TUI in an overlay pane. |
| Show Herduck work status | Prints Works from the local Herduck server. |

Optional keybinding in your Herdr config:

```toml
[[keys.command]]
key = "prefix+h"
type = "plugin_action"
command = "wenhanweime.herduck.open"
description = "open Herduck"
```

## Trust

Runs only the two shell scripts in this folder, then calls `herduck` / `jq` on your machine.
