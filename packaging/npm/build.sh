#!/bin/sh
# Assemble npm packages from release binaries.
#
#   packaging/npm/build.sh <version> <dist-dir>
#
# <dist-dir> must contain herduck-{macos,linux}-{x86_64,aarch64} binaries (as produced by
# .github/workflows/release.yml). Output goes to packaging/npm/out/.
set -eu

version="${1:?version required, e.g. 0.1.0-alpha.3}"
dist="${2:?dist dir required}"
here="$(cd "$(dirname "$0")" && pwd)"
out="$here/out"
rm -rf "$out"
mkdir -p "$out"

make_platform() {
  os="$1" cpu="$2" asset="$3"
  name="herduck-${os}-${cpu}"
  dir="$out/${os}-${cpu}"
  mkdir -p "$dir"
  cp "$dist/$asset" "$dir/herduck"
  chmod 755 "$dir/herduck"
  cat > "$dir/package.json" <<JSON
{
  "name": "$name",
  "version": "$version",
  "description": "Herduck prebuilt binary for $os-$cpu",
  "license": "AGPL-3.0-or-later",
  "repository": { "type": "git", "url": "https://github.com/wenhanweime/herduck.git" },
  "os": ["$os"],
  "cpu": ["$cpu"],
  "files": ["herduck"]
}
JSON
}

make_platform darwin arm64 herduck-macos-aarch64
make_platform darwin x64 herduck-macos-x86_64
make_platform linux arm64 herduck-linux-aarch64
make_platform linux x64 herduck-linux-x86_64

mkdir -p "$out/herduck"
cp -R "$here/herduck/bin" "$here/herduck/README.md" "$out/herduck/"
sed -e "s/\"version\": \"0.0.0\"/\"version\": \"$version\"/" \
    -e "s/\": \"0.0.0\"/\": \"$version\"/g" \
    "$here/herduck/package.json" > "$out/herduck/package.json"

echo "npm packages assembled in $out"
echo "Publish order: platform packages first, then herduck:"
for d in darwin-arm64 darwin-x64 linux-arm64 linux-x64 herduck; do
  echo "  npm publish --access public $out/$d"
done
