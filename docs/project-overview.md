# Project overview and follow-ups

Open a Work group or folder Project to read what its
Agents are doing and choose a next action. No plan has to be filled in first. The selected group
stays highlighted when viewing its detail, editing its plan, or opening a child session.

Each conversation appears once: its name, Agent, and status come first, followed by what is being
worked on or the latest recorded result. The next step and its controls sit directly below that
progress, so every action has a clear owner. Conversations waiting for your answer come first,
followed by ongoing work and quieter history. For example:

> **Working · Interview invitations · Pi**
>
> Checking the invitations and consent form.
>
> **Next:** Submit the consent form for review, then send the approved invitations.

Choose **Continue with this** (or the displayed number) to send that follow-up to its original
Agent and open the session. Herduck reuses a matching live pane; for history, it resumes the same
native conversation in its original working directory. If the Agent is busy, delivery waits for
its idle prompt. The pending action becomes **Cancel queued follow-up**. A sent state means the
instruction was delivered, not that the Agent completed the work.

**View conversation**, a conversation heading, or **Enter** open context without sending instructions
or starting a history session. A blocked Agent offers **Open to answer**, leaving the pending decision
to you. The saved goal and blocker provide context at the top; **edit plan** opens the complete authored plan.
Agent follow-ups take precedence over saved steps in the bounded suggestion list. Existing goals,
next steps, and blockers stay editable and are never overwritten by automatic updates.

Press **r** or click **refresh** to reread recent evidence. Use **↑/↓** and **Enter** to browse the session
sections, **e** to edit a Work plan, and **Esc** to return to the sidebar. Use the mouse wheel to scroll the same sections. Smaller terminals wrap the text and preserve each
visible action beside its owner. The footer shows the visible range and whether more conversations
are below; arrow keys bring the selected conversation and its controls into view. Folder Projects
share the overview; authored goals and plans remain a Work feature.

## Evidence and scope

The overview covers the newest 50 indexed conversations in the selected group, independently of the
All / Open filter. The filter controls which conversation sections are shown, and older paged records
remain accessible below the recent work. If older history exists, the status strip says **Latest 50**. An idle Agent
is ready for another instruction; a recorded conversation is history. Neither proves project completion.

The four newest conversations supply text evidence. A bounded background worker reads recent local
Codex, Claude, Pi, or Grok records, skipping tools, code blocks, and harness preambles. It uses meaningful
requests and readable responses rather than session titles. Explicit next-step sections and inline
recommendations in English or Chinese can supply follow-ups. Completed checkboxes are excluded, and a
newer user request supersedes an earlier Agent plan. If no explicit action was recorded, a continuation
asks the original Agent to identify and complete the next unfinished step using its existing context.

Evidence is cached for 15 seconds and refreshed as indexed activity changes. While it loads, runtime
states and conversation links remain usable. If a backend has no readable local transcript, the overview
says that a detailed update is unavailable. Evidence reads are local. Choosing a follow-up uses the
original Agent, with that Agent's existing tools and permissions.

## Language

`projects.summary.title_language` selects `en` or `zh` for automatic work descriptions, follow-up
recommendations, controls, states, and feedback. Continuation instructions explicitly request progress
updates and replies in that language. The language comes from the saved setting, not from individual
conversations; config reload applies it without restarting the server. The four sidebar tabs always
keep their English names: **Agents / Sessions / Projects / Work**. Narrow sidebars use two rows so
the names remain readable; the other controls follow the selected language.

When recent evidence is in another language, a bounded background worker translates it using the
configured **Summary** source order. Successful translations are cached by source content and language;
normal refreshes and runtime heartbeats do not repeat those calls. Old results cannot cross a language
or provider change. Off, Offline, and an empty Summary source list make no translation calls.

Until a valid translation is available, the overview shows a message in the selected language and a
read-only conversation link. It does not invent a recommendation or send an untranslated instruction.
Original transcripts, authored plans, names, paths, and commands retain their exact content. Switching
language does not create a second delivery for the same recommendation. An already queued instruction
keeps the wording the person selected; its delivery status follows the current language.

Delivery is bound to the original session, Agent, and terminal; replacing or closing that target cancels
pending input. At most one follow-up can wait per pane, with 16 pending and 128 retained delivery records
per server. These delivery records last for the running server's lifetime; they do not survive a restart
or live handoff. Saved Work plans and native conversation histories retain their existing persistence.

## CLI and socket API

With the matching server running, obtain a canonical key and read the overview:

```sh
herduck work list
herduck work overview get WORK_KEY
herduck project list
herduck project overview get PROJECT_KEY --refresh
```

These commands return JSON. Both overview commands use the public method `project.overview.get`:

```json
{
  "id": "read-progress",
  "method": "project.overview.get",
  "params": {"project_key": "CANONICAL_KEY", "refresh": false}
}
```

The result has `type: "project_overview"` and an `overview` object with `language` (`en` or `zh`), phase `counts`,
`observed_sessions`, `more_history_available`, `work`, and `suggestions`. Work items carry a prose
`description`, `session_key`, `last_activity_at`, quote `update_origin`, and `evidence_read_at`
(Unix milliseconds). Suggestions retain their `id`, `description`, `source`, `reason`, optional
`session_key`, executable `prompt`, and any current `followup` delivery record.

When `activity_loading` is true, read again after evidence or translation work finishes. `refresh: true`
requests fresh evidence without blocking on file or provider work. Reads do not change UI selection, saved plans,
Catalog revision, or running processes.

After choosing a suggestion, use its exact `id` from the overview:

```sh
herduck work followup start WORK_KEY SUGGESTION_ID
herduck work followup get FOLLOWUP_ID
herduck work followup cancel FOLLOWUP_ID
```

The same commands work under `herduck project followup`. Public methods are:

| Method | Parameters | Result |
| --- | --- | --- |
| `project.followup.start` | `project_key`, `suggestion_id` | Start or return the existing delivery |
| `project.followup.get` | `followup_id` | Read delivery state |
| `project.followup.cancel` | `followup_id` | Cancel input that has not been sent |

Responses have `type: "project_followup"` and `followup` with an `id`, project/session/pane identities,
`title`, `state` (`queued`, `sent`, or `cancelled`), `message`, and `created_at`. Repeating a start with
the same retained ID returns its existing result without sending again. A changed suggestion returns
`conflict`; unknown group or delivery IDs return `not_found`. Suggestions without a prompt must be
opened for an answer or plan edit. The [bundled schema](api/herduck-api.schema.json) describes the contract.
