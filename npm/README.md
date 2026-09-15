# Herduck

Herduck is a terminal workspace for people and agents. This package launches the native
application on macOS and Linux (x64 and arm64).

Node.js 20+ is required; Rust and Zig are not. Install and sign in to your Agent CLIs separately.

```sh
npm install -g herduck@alpha
herduck
```

Or run without a global installation:

```sh
npx --yes herduck@alpha
```

The launcher downloads the matching binary from its GitHub release and verifies the SHA-256
hash against the package manifest. Subsequent runs reuse the verified binary and work offline.
Agent services may still require a network connection. No npm install scripts or runtime
dependencies are used.

Binaries are stored in `${XDG_DATA_HOME:-~/.local/share}/herduck/runtime/<version>/`.
`npm uninstall -g herduck` removes the launcher, while configuration and conversations remain.
Stop the associated servers before manually deleting runtime files.

Linux prebuilt binaries require glibc 2.39+. Alpine/musl and Windows are not supported.
For version pinning, source installation, signing, and upgrades, see the
[installation guide](https://github.com/wenhanweime/herduck/blob/main/docs/installation.md).

Herduck is derived from Herdr v0.7.4 and distributed under AGPL-3.0-or-later.
The commercial-license offer in the preserved LICENSE applies to upstream Herdr, not Herduck's
modifications. See [source and provenance](https://github.com/wenhanweime/herduck/blob/main/docs/UPSTREAM.md).

[Product and documentation](https://github.com/wenhanweime/herduck)
