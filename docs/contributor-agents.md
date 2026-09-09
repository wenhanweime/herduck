# Contributor-agent guidance

This guidance applies to AI-assisted contributions to Herduck. Read [CONTRIBUTING.md](../CONTRIBUTING.md)
for the contribution process and [SECURITY.md](../SECURITY.md) before handling vulnerability reports.
The human contributor remains responsible for understanding the change.

## Scope and collaboration

- Work against the intended Herduck repository and check the configured Git remote before pushing.
- Agree on the contribution scope before making broad changes. Preserve unrelated work in the checkout.
- Agents must not submit issues on a human's behalf. Help the human prepare a minimal, redacted report.
- Follow the first-contribution approval process in `CONTRIBUTING.md` before opening a PR.
- Never publish credentials, personal configuration, session histories, logs, or private machine paths.
- Use lowercase conventional commits, without emojis or AI co-author lines. Agree on the message before committing.

## Architecture and compatibility

- Keep `AppState` and `PaneState` as pure data, separate from PTYs and runtime resources.
- Keep rendering pure: compute geometry and state changes before drawing.
- Put shared session facts in runtime/server state and expose them through the public API when practical.
  Keep selection, layout, dialogs, and other presentation state in the TUI client.
- Isolate OS behavior in `src/platform/` and compile-gate platform-specific code.
- Reuse existing dialogs and input patterns. Avoid modules that accumulate unrelated responsibilities.
- Preserve persisted identity, protocol contracts, integration names, and state compatibility deliberately.
  For broad or release-risk changes, identify protected behavior and review the design before editing.
- For detection manifest changes, capture the detection buffer with
  `herduck agent read <pane> --source detection --format text`; use ANSI output when styling matters.
  Match visible invariant controls and inspect the result with `herduck agent explain <pane> --json`.
- Preserve upstream attribution, licenses, and the vendored patch records.

## Implementation and verification

Use the pinned toolchain and `just` recipes. Production Rust should handle errors without `unwrap()`;
use `tracing` for diagnostics. Avoid new dependencies unless existing ones do not meet the need.

Before sharing a change, run:

```bash
just test-public
just check
```

Add meaningful regression coverage for changed behavior. State and identity tests should avoid real
PTYs where possible; use the existing test constructors and invariants. Test real runtime or install
behavior when the change affects those paths. Do not weaken assertions to make a failing check pass.

Report what changed, why, the checks actually run, and any unresolved limitations. Keep local plans
and test artifacts under ignored `.local/prd/`. Public documentation belongs in `README.md` and `docs/`.
Use Herduck's own source-release process; never invoke inherited upstream publishing automation.
