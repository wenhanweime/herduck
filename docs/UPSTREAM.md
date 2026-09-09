# Upstream provenance

Herduck is an independent product and repository. Its terminal runtime was originally derived from
Herdr v0.7.4 and has since been modified to provide project and semantic-cluster navigation,
configuration, sockets, commands, and product identity. The project was previously named ORK3
and was renamed Herduck in September 2026. It is independently maintained and is not an official
Herdr release.

| Field | Value |
|---|---|
| Upstream repository | `https://github.com/ogulcancelik/herdr.git` |
| Imported release | `v0.7.4` |
| Release commit | `50aaa2ec046ee26ff407c20f49de496f522512a8` |
| Annotated tag object | `54208dc16efe15ea92d7f131439d43cbd84b489e` |
| Original subtree import commit | `c373ec9` |
| Source archive SHA-256 | `9a91b2da04831484e9174183832a0884836e4df05756ce58d529479e2e73699f` |
| License | `AGPL-3.0-or-later` |
| Imported license SHA-256 | `a7fa24f74382fb3e4d320a608533a7c2999dbc0f780f1f734c8b891b31f0d9bd` |

The archive checksum is the SHA-256 of `git archive --format=tar` for the release commit. The
original copyright notices are preserved in source files and [LICENSE](../LICENSE). References to
Herdr elsewhere in implementation internals indicate protocol or compatibility ancestry; they do
not mean that Herduck depends on a separately installed Herdr executable.

Herduck is distributed under AGPL-3.0-or-later. The commercial-license offer in the preserved
upstream `LICENSE` refers to Herdr and its original rights holder; it is not a separate commercial
license for Herduck's modifications.

Vendored dependencies retain their own notices, including [Ghostty](../vendor/libghostty-vt/LICENSE)
and [portable-pty](../vendor/portable-pty/LICENSE.md). Their source is included in the Git checkout
used for installation. Herduck keeps some historical integration names and serialized values for
compatibility with existing sessions and hooks; those identifiers are not product branding.
