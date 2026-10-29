# How to keep track of many Claude Code (and Codex) sessions

[简体中文](manage-many-claude-code-sessions.zh-CN.md)

*Written by the Herduck author. Most of this page works with no extra tools at all; Herduck only
comes up near the end. Facts about Claude Code come from its own documentation (linked at the
bottom), checked on 2026-10-30.*

## The problem

You start one Claude Code session to fix a bug, another to write tests, a Codex session for a
refactor, and one more to look into something odd in another folder. By the fifth terminal you no
longer remember which window is doing what, which one is waiting for you, and — the next morning —
what each piece of work was for and what was supposed to happen next.

There are three separate problems hiding in there:

1. **Finding a session again** (which conversation was that?)
2. **Seeing which session needs you right now** (who is waiting for an answer?)
3. **Remembering the work itself** (what was the goal, what is blocking it, what is next?)

The free tools below are good at the first two. The third one is usually left to you.

## 1. Give every session a name

Unnamed sessions show up in lists as the first line of your prompt, which is hard to recognise
later. Name them:

```sh
claude -n auth-refactor          # name it when you start
# or, inside a running session:
/rename auth-refactor
```

Then you can come back to it directly:

```sh
claude --resume auth-refactor    # reopen that session by name
claude --resume                  # or pick from a list
claude --continue                # or just reopen the latest one in this folder
```

## 2. One folder (git worktree) per task

If two sessions edit the same files at the same time, they get in each other's way. A git
worktree is a second copy of your repository on its own branch, so each session gets its own
files. Claude Code can create one for you:

```sh
claude --worktree feature-auth
```

Run the same command with another name in another terminal for a second, separate session.

## 3. One place to see who needs you

Recent Claude Code versions have **agent view**: run `claude agents` to see your background
Claude Code sessions in one screen — which are working, which are waiting for your input, which
are done. Send a running session to the background with `/bg`.

If you prefer plain terminals, tmux keeps sessions alive in named windows when you close the
terminal or lose SSH (`tmux new -s shop`, `Ctrl-b d` to detach, `tmux attach -t shop` to come
back). tmux does not know what an Agent is doing, so you still have to look at each window.

## 4. Write down the work, not just the session

This is the part the tools above do not do for you. A session is one conversation; a piece of
work often spans several conversations, several Agents, and several days. A habit that helps
regardless of tools: keep a short note per piece of work with four lines —

```text
Goal:     what "done" looks like
Status:   where it stands right now
Blocker:  what is stopping it, if anything
Next:     the next one to three steps
```

— and update it when you stop for the day. A text file in the project folder is enough.

## Where Herduck fits

If you only use Claude Code, in one or two folders, the steps above are probably all you need.

Herduck is for the case where that note-keeping gets out of hand: several Agents (Claude Code,
Codex, OpenCode, Pi, …) across several folders, with work that lasts days. What it does, per its
[README](../README.md):

- **Work view**: keeps an editable goal, up to three next steps, and a blocker note per piece of
  work. These survive sessions ending and the server restarting, and automatic updates never
  overwrite what you wrote. Conversations from different Agents and folders can be grouped
  under one Work.
- **Sessions view**: one searchable list of local conversation histories from Claude Code, Codex,
  Pi, OpenCode, and Grok — including ones you never started inside Herduck. Preview without
  launching anything; continue with the original Agent when supported.
- **Agents view**: Agent terminals side by side, with working / waiting / idle states and
  configurable notifications. Close the client and the server keeps them running; run
  `herduck` again to reattach.
- **Continue with this**: sends a chosen next step to the original Agent, reusing its live
  session or resuming the original conversation.

What to know before trying it:

- It is an **alpha**, macOS and Linux only. The Work features need a build from source; the npm
  package is older (see [installation](installation.md)).
- Automatic Work grouping, session names, and progress descriptions are **off by default** and
  need a model source you configure (OpenCode, Pi, Codex, Hermes CLI, or an OpenAI-compatible
  API). When enabled, selected conversation content is sent to that source. With them off, the
  Agents, Sessions, and Projects views still work, and reading local history makes no model calls.
- It does not create git worktrees for you; keep using `claude --worktree` or `git worktree`.

How it compares with `claude --resume`, tmux, Herdr, and other session managers:
[compare-resume-tmux.md](compare-resume-tmux.md).

## Sources

- Claude Code: [Manage sessions](https://code.claude.com/docs/en/sessions) ·
  [Worktrees](https://code.claude.com/docs/en/worktrees) ·
  [Agent view](https://code.claude.com/docs/en/agent-view)
- tmux: <https://github.com/tmux/tmux/wiki>
- Herduck: [README](../README.md) · [project overview](project-overview.md) ·
  [configuration](configuration.md)
