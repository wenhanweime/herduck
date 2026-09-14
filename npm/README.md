# Herduck

**The persistent work layer for agents.**

Built on [Herdr](https://github.com/ogulcancelik/herdr), Herduck brings familiar tmux window
management together with Agent activity and a conversation library across your device.
Arrange terminals and browse related conversations through Work, Projects, and Sessions.
People use the UI; Agents can inspect and manage the workspace through the CLI and JSON API.
Work is the new name for Topics in GitHub source; the published npm alpha still uses Topics.
Saved plans and executable follow-ups are available in the [source preview](https://github.com/wenhanweime/herduck/pull/1).

Version **0.1.0-alpha.4** describes current work from local conversations and offers concrete
follow-ups. Choose **Continue with this** to send a suggestion to its original Agent, or
**View conversation** to inspect the context. Selected Work groups and Projects stay highlighted;
Work goals, next steps, and blockers remain editable.
This source preview is awaiting publication; `herduck@alpha` follows the latest published release.

This package launches the native Herduck application on macOS and Linux (x64 and arm64).
Node.js 20+ is required; Rust and Zig are not. Your agent CLIs are installed separately.

Install the current alpha from npm:

```sh
npm install -g herduck@alpha
herduck
```

Or run without a global npm install:

```sh
npx --yes herduck@alpha
```

On first run, the launcher downloads the matching binary from that exact GitHub release and
verifies its SHA-256 against the manifest embedded in this package. Subsequent runs work offline.
No npm install scripts or runtime dependencies are used. The unbuilt source manifest deliberately
contains no binaries; use the packaged release `.tgz`, not `npm install` from the source directory.

Binaries stay in `${XDG_DATA_HOME:-~/.local/share}/herduck/runtime/<version>/` so the persistent
server can restart independently of npm's cache. `npm uninstall -g herduck` removes the launcher;
stop your Herduck servers before manually deleting old runtime versions. Your configuration and
conversations are not removed. On Linux, prebuilt binaries require glibc 2.39+; Alpine/musl is not
supported by these downloads.

This is an alpha preview. Use `herduck@0.1.0-alpha.4` to pin this version; repeat
`npm install -g herduck@alpha` to upgrade to the current alpha. The same version's archive is also
available from [GitHub Releases](https://github.com/wenhanweime/herduck/releases/tag/v0.1.0-alpha.4).

Herduck is an independent project derived from Herdr v0.7.4 and distributed under
AGPL-3.0-or-later. The commercial-license offer in the preserved LICENSE applies to upstream
Herdr, not Herduck's modifications. [Source and provenance for this version](https://github.com/wenhanweime/herduck/blob/v0.1.0-alpha.4/docs/UPSTREAM.md)
include the upstream revision and vendored dependency notices.

[Screenshots, configuration, and source](https://github.com/wenhanweime/herduck)
