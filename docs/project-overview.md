# Project overview and follow-ups

Available in the **0.1.0-alpha.4 source preview**. Open a Topic or folder Project to read what its
Agents are doing and choose a next action. No plan has to be filled in first. The selected group
stays highlighted when viewing its detail, editing its plan, or opening a child session.

**What's happening** shows the concrete request being worked on or the latest recorded result.
The session name, Agent, and state sit below the description, so you can tell where it came from.
**Suggested follow-ups** quotes next actions from the conversation, with their context. For example:

> The Agent recommends submitting the tested change for review, then redeploying after approval.

Choose **Continue with this** (or the displayed number) to send that follow-up to its original
Agent and open the session. Herduck reuses a matching live pane; for history, it resumes the same
native conversation in its original working directory. If the Agent is busy, delivery waits for
its idle prompt. The pending action becomes **Cancel queued follow-up**. A sent state means the
instruction was delivered, not that the Agent completed the work.

**View conversation** and the conversation list open context without sending instructions or
starting a history session. A blocked Agent offers **Open to answer**, leaving the pending decision
to you. A saved step without a conversation link opens the Topic plan editor. Existing goals,
next steps, and blockers stay editable and are never overwritten by automatic updates.

Press **r** or click **refresh** to reread recent evidence. Use **↑/↓** and **Enter** for the conversation
list, **e** to edit a Topic plan, and **Esc** to return to the sidebar. Smaller terminals show fewer
descriptions while keeping follow-up controls and conversation navigation reachable. Folder Projects
share the overview; authored goals and plans remain a Topic feature.

## Evidence and scope

The overview covers the newest 50 indexed conversations in the selected group, independently of the
conversation-list filter. If older history exists, the status strip says **Latest 50**. An idle Agent
is ready for another instruction; a recorded conversation is history. Neither proves project completion.

The four newest conversations supply text evidence. A bounded background worker reads recent local
Codex, Claude, Pi, or Grok records, skipping tools, code blocks, and harness preambles. It uses meaningful
requests and readable responses rather than session titles. Explicit next-step sections and inline
recommendations in English or Chinese can supply follow-ups. Completed checkboxes are excluded, and a
newer user request supersedes an earlier Agent plan. If no explicit action was recorded, a continuation
asks the original Agent to identify and complete the next unfinished step using its existing context.

Evidence is cached for 15 seconds and refreshed as indexed activity changes. While it loads, runtime
states and conversation links remain usable. If a backend has no readable local transcript, the overview
says that a detailed update is unavailable. Reading the overview makes no model or network requests.
Choosing a follow-up uses the original Agent, with that Agent's existing tools and permissions.

Delivery is bound to the original session, Agent, and terminal; replacing or closing that target cancels
pending input. At most one follow-up can wait per pane, with 16 pending and 128 retained delivery records
per server. These delivery records last for the running server's lifetime; they do not survive a restart
or live handoff. Saved Topic plans and native conversation histories retain their existing persistence.

## CLI and socket API

With the matching server running, obtain a canonical key and read the overview:

```sh
herduck topic list
herduck topic overview get TOPIC_KEY
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

The result has `type: "project_overview"` and an `overview` object with phase `counts`,
`observed_sessions`, `more_history_available`, `work`, and `suggestions`. Work items carry a prose
`description`, `session_key`, `last_activity_at`, quote `update_origin`, and `evidence_read_at`
(Unix milliseconds). Suggestions retain their `id`, `description`, `source`, `reason`, optional
`session_key`, executable `prompt`, and any current `followup` delivery record.

When `activity_loading` is true, read again after the background worker finishes. `refresh: true`
requests fresh evidence without blocking on file reads. Reads do not change UI selection, saved plans,
Catalog revision, or running processes.

After choosing a suggestion, use its exact `id` from the overview:

```sh
herduck topic followup start TOPIC_KEY SUGGESTION_ID
herduck topic followup get FOLLOWUP_ID
herduck topic followup cancel FOLLOWUP_ID
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
