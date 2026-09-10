# Herduck

**Your AI coding agents. One terminal.**

Run agents side by side, find past conversations, and organize work by project and topic.

This package launches the native Herduck application on macOS and Linux (x64 and arm64).
Node.js 20+ is required; Rust and Zig are not. Your agent CLIs are installed separately.

Install the release package:

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

This is an alpha preview. Pin `herduck@0.1.0-alpha.2` for this exact release. A direct
[GitHub package](https://github.com/wenhanweime/herduck/releases/download/v0.1.0-alpha.2/herduck-0.1.0-alpha.2.tgz)
is available too; pass its URL to `npm install -g`, or use
`npx --yes --package=<package-url> herduck`.

Herduck is an independent project derived from Herdr v0.7.4 and distributed under
AGPL-3.0-or-later. The commercial-license offer in the preserved LICENSE applies to upstream
Herdr, not Herduck's modifications. [Source and provenance for this version](https://github.com/wenhanweime/herduck/blob/v0.1.0-alpha.2/docs/UPSTREAM.md)
include the upstream revision and vendored dependency notices.

[Screenshots, configuration, and source](https://github.com/wenhanweime/herduck)
