# Work plans

Work was previously labelled Topics. The existing `herduck topic` commands and `topic.*` API
names remain compatible; this name change does not migrate saved plans or conversation identities.

Introduced in the **0.1.0-alpha.3 source preview**. A saved Work plan contains this week's goal,
up to three next steps, and what's blocked. In **0.1.0-alpha.4**, the [project overview](project-overview.md)
shows each conversation's progress and next action together. The saved goal and blocker stay at the
top as context, while the complete plan is available through **edit plan**. An empty plan takes no
space. Agent follow-ups have a separate **Continue with this** action and take precedence over saved
plan items in the overview's suggestion list.

Click **edit plan** or press **e** in the Work detail view. Use Tab / Shift-Tab to move between
fields, Shift-Enter for a new line, and Enter or the **save** button to save. Esc cancels the
draft. Ctrl-U clears the current field. The editor supports Chinese input and pasted text.
On smaller terminals, it scrolls the form to the focused field while keeping the action buttons visible.

The goal and blocker stay fixed while the conversation sections below them scroll. Click a conversation or select it
with the arrow keys and press Enter to use the existing history-preview or live-session controls.
Esc returns to the Work list. Editing a cover does not start an Agent.

## CLI

With the matching Herduck server running, list Work groups and copy the desired `canonical_key`:

```sh
herduck topic list
herduck topic cover get TOPIC_KEY
herduck topic cover update TOPIC_KEY \
  --goal "Agree on a testable outcome this week" \
  --next-step "Review the current progress" \
  --next-step "Decide who handles the next step" \
  --blocked-note "Waiting for feedback"
```

All commands return JSON. Replace `TOPIC_KEY` with the stable key from the list. Named sessions
use the existing `--session NAME` option. The CLI uses the same socket API and Catalog writer as
the editor.

Updates affect only the fields you supply. Repeated `--next-step` options replace the next-step
list; they do not append to it. To clear values explicitly:

```sh
herduck topic cover update TOPIC_KEY --goal ""
herduck topic cover update TOPIC_KEY --clear-next-steps --blocked-note ""
```

For a guarded update, pass `--expected-updated-at N`, using `updated_at` from the last read
(`0` for a cover that has never been saved). If the cover changed in the meantime, the update
fails with `topic_cover_conflict` and leaves the newer cover intact. The UI always uses this guard:
on conflict, the draft stays open so you can review it and reopen the latest cover before saving.

## Socket API

`topic.cover.get` reads a cover:

```json
{"id":"cover-read","method":"topic.cover.get","params":{"topic_key":"TOPIC_KEY"}}
```

The result has `type: "topic_cover"`, `topic_key`, and `cover`. A cover contains `goal`,
`next_steps`, `blocked_note`, `blocked_session_ref`, and `updated_at` (Unix milliseconds).

`topic.cover.update` applies a partial patch:

```json
{
  "id": "cover-write",
  "method": "topic.cover.update",
  "params": {
    "topic_key": "TOPIC_KEY",
    "patch": {
      "goal": "A revised goal",
      "expected_updated_at": 0
    }
  }
}
```

Successful updates return `type: "topic_cover_updated"` and the new Catalog `revision`.
They also publish `project.snapshot.updated`; `project.snapshot` includes the saved `cover`
on its Work entry. Running clients refresh from that shared snapshot.

At most three next steps are accepted, and each text field is limited to 2,000 Unicode characters.
Newlines and tabs are allowed; terminal control characters are rejected. Text is trimmed and blank
next-step slots are omitted. Invalid updates fail atomically with `invalid_topic_cover`.
Unknown Work keys and directory-project keys return `not_found`. Unknown patch fields are rejected.
The complete contract is included in [the API schema](api/herduck-api.schema.json).

## Persistence and scope

Covers are stored in the existing per-session Catalog (`projects/catalog.sqlite3` under the
[session data directory](configuration.md#paths)). The v7 migration adds a separate `topic_covers`
table. Existing conversations and group identities are preserved; older groups and snapshots without
a cover read as empty. Saved covers survive client detachment and server restart.

Classification and model-based merging never rewrite or merge authored cover text. If a
group's last conversation moves to another Work group, its saved cover remains accessible under its
original name. A cover is not inferred from chat content.

Authored blockers remain text; `blocked_session_ref` is reserved and remains `null` through the
cover editor/API. The automatic overview separately links an Agent waiting for input to its
existing pane. Artifact lists, task stages, approval flows, and multi-user permissions remain out of scope.
