# Installation

Herduck is an alpha preview for macOS and Linux. Install your coding-agent CLIs and complete their
login separately; Herduck does not supply an Agent subscription or credentials.

This source branch targets **0.1.0-alpha.3**. The version-pinned npm packages, release URLs, and
Git tag below become available when that release is published. Until then, `herduck@alpha` installs
the latest published alpha; use this checkout's source build to try [Topic covers](topic-covers.md).

## npm and npx

Node.js 20 or later is required for the launcher. Rust and Zig are not required for prebuilt binaries.
Install the current alpha from npm:

```sh
npm install -g herduck@alpha
herduck
```

To try the same release without a global npm installation:

```sh
npx --yes herduck@alpha
```

The `alpha` tag follows alpha releases. To pin this release, use
`npm install -g herduck@0.1.0-alpha.3` or `npx --yes herduck@0.1.0-alpha.3`.

The first launch downloads the matching binary from the exact GitHub release and checks its SHA-256
against the package's embedded manifest. Subsequent launches reuse the verified binary and can work offline. Agent
services may still need a network connection. There are no npm lifecycle install scripts or
runtime dependencies.

If npm reports a permission error on global installation, use npx or a Node installation owned by
your user. Do not run Herduck with `sudo`.

### Install from GitHub

The identical npm archive is also attached to the GitHub release. These URLs pin version `0.1.0-alpha.3`:

```sh
npm install -g https://github.com/wenhanweime/herduck/releases/download/v0.1.0-alpha.3/herduck-0.1.0-alpha.3.tgz
herduck
```

Or use npx:

```sh
npx --yes --package=https://github.com/wenhanweime/herduck/releases/download/v0.1.0-alpha.3/herduck-0.1.0-alpha.3.tgz herduck
```

## Supported prebuilt platforms

| System | CPU | Release asset | Validation environment |
| --- | --- | --- | --- |
| macOS | Apple silicon | `herduck-macos-aarch64` | macOS 15 |
| macOS | Intel | `herduck-macos-x86_64` | macOS 15 |
| Linux | x64 | `herduck-linux-x86_64` | Ubuntu 24.04, glibc 2.39 |
| Linux | arm64 | `herduck-linux-aarch64` | Ubuntu 24.04, glibc 2.39 |

Linux downloads require glibc 2.39 or later. Older Linux distributions need a source build;
Alpine/musl and Windows are not supported by these prebuilt downloads. Earlier macOS versions
have not been validated. macOS binaries use ad hoc signing, not Apple notarization.

## Direct binaries

Download the asset for your CPU and `SHA256SUMS` from the
[release](https://github.com/wenhanweime/herduck/releases/tag/v0.1.0-alpha.3). In the download directory,
verify that asset before installation. For example, on Apple silicon:

```sh
shasum -a 256 herduck-macos-aarch64
# Compare the complete digest with its entry in SHA256SUMS.
mkdir -p "$HOME/.local/bin"
install -m 755 herduck-macos-aarch64 "$HOME/.local/bin/herduck"
"$HOME/.local/bin/herduck" --version
```

On Linux, `sha256sum` can check the corresponding Linux asset. Keep the installation directory on
`PATH`. Browser downloads on macOS may be quarantined; use the system's normal Privacy & Security
approval only after checking the source and hash, or use the npm launcher.

## Build from source

Source builds require **Rust 1.96.1 through rustup** (pinned in `rust-toolchain.toml`),
**Zig 0.15.2**, Git, and the platform build tools.

### macOS build tools

```sh
xcode-select --install # if Apple command-line tools are not installed
brew install rustup zig@0.15
rustup-init
export ZIG="$(brew --prefix zig@0.15)/bin/zig"
"$ZIG" version # must report 0.15.2
```

### Debian / Ubuntu build tools

```sh
sudo apt-get update
sudo apt-get install -y build-essential curl git ca-certificates xz-utils
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/herduck-rustup.sh
sh /tmp/herduck-rustup.sh
. "$HOME/.cargo/env"
rustup toolchain install 1.96.1
```

Use rustup's Cargo on `PATH`; older distribution Rust packages cannot build this release.
Install [Zig 0.15.2](https://ziglang.org/download/) for your architecture:

```sh
case "$(uname -m)" in
  x86_64) zig_arch=x86_64 ;;
  aarch64|arm64) zig_arch=aarch64 ;;
  *) echo "See ziglang.org/download for your architecture"; exit 1 ;;
esac
mkdir -p "$HOME/.local/opt"
curl -fL "https://ziglang.org/download/0.15.2/zig-${zig_arch}-linux-0.15.2.tar.xz" \
  -o /tmp/herduck-zig.tar.xz
tar -xJf /tmp/herduck-zig.tar.xz -C "$HOME/.local/opt"
export ZIG="$HOME/.local/opt/zig-${zig_arch}-linux-0.15.2/zig"
"$ZIG" version # must report 0.15.2
```

Keep `ZIG` set in the build shell. On other Linux distributions, install the equivalent build tools
and `xz`. An unversioned package-manager install may select an incompatible newer Zig.

### Compile and install

```sh
git clone --branch v0.1.0-alpha.3 https://github.com/wenhanweime/herduck.git
cd herduck
cargo install --path . --locked
```

On macOS, re-sign the locally built executable after installation or replacement:

```sh
codesign --force --sign - "${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin/herduck"
```

`cargo install` puts the executable in Cargo's bin directory, normally `~/.cargo/bin`.
Ensure it is on `PATH`, then run `herduck`. The complete checkout is required, including vendored
dependencies. This project is not published to crates.io; `cargo install herduck` is not supported.

For development without installing, use `cargo run --locked -- --help` or `cargo run --locked`.
Repository recipes additionally require `just`, `zsh`, Python 3, and Node.js 20+.
Creating release archives with `tooling/package_release.py` requires Python 3.11+ and all four
verified native assets. The checked-in npm manifest is intentionally empty; do not publish or
install the raw `npm/` directory. CI generates the distributable manifest from the built binaries.

## Upgrade, storage, and removal

To upgrade an npm installation to the current alpha, including one originally installed from a
GitHub archive:

```sh
npm install -g herduck@alpha
```

For a pinned upgrade, replace `alpha` with the desired version, or use the GitHub package URL from
that release. A different release gets a different native runtime directory; npm does not overwrite
the executable of a running server.
`herduck --version` reports the installed client. An already running server keeps its current
version until handed off or stopped. Finish active work before stopping a server; stopping it ends
its terminals. Do not delete old runtime files while those servers are still running.

The npm launcher keeps verified executables under
`${XDG_DATA_HOME:-~/.local/share}/herduck/runtime/<version>/`, outside npm's disposable cache.
`npm uninstall -g herduck` removes the launcher. It does not remove runtime files, configuration,
or conversation history. Data locations and socket overrides are documented in
[configuration](configuration.md#paths).

Self-update remains disabled. Source installs upgrade by checking out a reviewed newer release
and repeating the source installation and, on macOS, signing steps above.
