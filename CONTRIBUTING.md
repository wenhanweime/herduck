# Contributing to Herduck

Herduck is an independent fork of Herdr. Send Herduck contributions to this repository;
upstream Herdr has its own contribution process. Preserve the original license and
copyright notices. See [upstream provenance](docs/UPSTREAM.md).

## Discuss the scope first

Use GitHub Discussions for feature requests, questions, design changes, and contribution
proposals. Reproducible bugs belong in the issue tracker. Before a first PR, obtain maintainer
approval on an accepted issue. If Discussions or the approval process is not yet configured,
wait for the repository maintainer to provide a contribution path.

AI assistance is welcome, but you must understand and be able to explain the submitted code.
Agents must follow the published [contributor-agent guidance](docs/contributor-agents.md) and must
not submit issues on a human's behalf. They may help draft reports for the human to review and submit.

A bug report should include the shortest reproduction, current and expected behavior, impact,
Herduck version or commit, operating system, terminal, and relevant configuration. Redact credentials,
private paths, and conversation content. Report security vulnerabilities privately through
[SECURITY.md](SECURITY.md).

## Development

Follow the pinned Rust and Zig setup in [README.md](README.md#requirements). Install `just`,
`zsh`, and Python 3 to run the repository recipes. The initial platform scope is macOS and Linux;
Windows support requires separate validation.

```bash
just build
just test --bin herduck <test-name-filter>
just test-public
just check
```

`just check` runs formatting, public-source portability checks, Clippy, and Rust tests. Do not
bypass failures. Include the checks actually run and any unresolved limitations in the PR.
Debug builds use a separate `herduck-dev` namespace, or reuse existing `ork3-dev` data in place.
When running inside Herduck, clear inherited socket overrides to keep the development client
attached to the development runtime:

```bash
env -u HERDUCK_SOCKET_PATH -u HERDUCK_CLIENT_SOCKET_PATH \
    -u ORK3_SOCKET_PATH -u ORK3_CLIENT_SOCKET_PATH \
    -u HERDR_SOCKET_PATH -u HERDR_CLIENT_SOCKET_PATH cargo run --locked
```

Keep runtime state separate from TUI presentation state; rendering must remain pure. Add focused
regression coverage for changed behavior. Put machine-specific preferences in local configuration,
not in source defaults or fixtures. Run `just check-public` before sharing changes; it is a
portability check, not a complete secret audit.

## Documentation and review

Keep changes focused and explain the user-visible behavior and validation. Update `README.md` and
the relevant files under `docs/` when the documented source-install workflow or behavior changes.
Local plans and exploratory notes belong under ignored `.local/prd/`; include the information a
reviewer needs in the contribution itself.

Use lowercase conventional commit subjects, without emojis or AI co-author lines. Agree on the
commit message before committing. For related issues, use `refs #<number>` in the body; do not
use automatic closing keywords for unreleased work.

Herduck currently installs from source. Do not invoke upstream release automation, publish upstream
assets, or enable self-update as part of an ordinary contribution. A Herduck release must identify
the exact reviewed commit and the platforms actually validated. Registry packages and prebuilt
binaries need their own packaging and installation validation.
