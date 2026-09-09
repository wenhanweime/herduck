---
title: Session library
description: Open sessions and a browsable conversation library.
---

The sidebar has four peer tabs in a single row: **Agents**, **Sessions**,
**Projects**, and **Topics**. **Agents** opens the workspace and pane layout
controls. The other tabs browse sessions by recency, project, or topic, with
**all** and **open** filters above the selected tab's session list.

The tabs use bold, centered labels and an accent-filled active tab in one terminal
row; the entire tab is clickable.

Idle agents become **inactive** after an hour without input, output, or state changes.
They remain visible in **Agents** with a muted status. The process, terminal, and
conversation stay alive: focus the same session and type to continue. New activity
clears the inactive status. Working agents and agents awaiting approval are not marked
inactive. `[session] agent_idle_timeout_secs` controls this threshold; `0` disables
inactivity marking. This timeout never closes a session or terminates its agent.

**Topics** groups sessions by their meaning across directories and agents. New
sessions enter Topics after semantic classification; directory names, agent names,
and title prefixes do not create placeholder topics. If classification is unavailable,
existing topics remain available and unclassified sessions wait for a later pass.
Local summary mode generates titles while retaining existing semantic topics.

Topics are ordered by their most recently active session. Sessions within each topic
also run newest first, including short conversations and older pages. Focusing a
session does not give its topic a higher sorting priority.

Live sessions in **Sessions**, **Projects**, and **Topics** keep two lines with a filled
background, even after focus moves elsewhere. The first line retains the green marker;
the second shows the agent status and working directory. The current session also has
an accent bar. Both lines belong to the same click target.

Focusing a session does not promote its Project or Topic above newer history.
Explicitly collapsed groups remain collapsed. Automation remains excluded or
collapsed. Configured disposable OpenCode naming workers are excluded when a dedicated
temporary worker directory, untouched default title, and at most two user turns agree.
Mentioning a worker or having a short conversation alone does not hide a session.
Source history is retained; explicit locked Catalog assignments are respected.

Clicking a historical session opens detailed readable context without starting an
agent. In its input box, Enter with no message resumes the original session without
sending an empty prompt. Enter with text resumes and sends that text when the agent
is ready. Shift+Enter inserts a newline. A matching running instance is reused;
failed resume attempts keep the draft and show the reason.

History reads remain bounded; a truncation notice means that earlier turns were not
loaded, not that the provider deleted them. Monolithic `--no-session` mode uses
the **Agents** tab because it does not start the session Catalog.

The four navigation tabs use a compact single terminal row with centered labels.
The Agents view keeps its `spaces` section heading below the tabs. Every expanded
sidebar view shows `settings / menu`; use it for Settings, keybinds, configuration
reload, and detach. In Sessions, Projects, and Topics, this menu sits below the list.
