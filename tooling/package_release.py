#!/usr/bin/env python3
"""Pack the dependency-free npm launcher with hashes of the native release assets."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

ASSETS = {
    "darwin-x64": "herduck-macos-x86_64",
    "darwin-arm64": "herduck-macos-aarch64",
    "linux-x64": "herduck-linux-x86_64",
    "linux-arm64": "herduck-linux-aarch64",
}


def package_release(root, assets, output):
    version = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
    package = json.loads((root / "npm/package.json").read_text())
    if version != package["version"]:
        raise ValueError("Cargo and npm versions must match")
    manifest = {"version": version, "platforms": {}}
    checksums = []
    for platform, name in ASSETS.items():
        asset = assets / name
        if not asset.is_file() or asset.stat().st_size == 0:
            raise ValueError(f"Missing or empty native asset: {name}")
        with asset.open("rb") as handle:
            digest = hashlib.file_digest(handle, "sha256").hexdigest()
        manifest["platforms"][platform] = {
            "url": f"https://github.com/wenhanweime/herduck/releases/download/v{version}/{name}",
            "sha256": digest,
        }
        checksums.append(f"{digest}  {name}\n")
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="herduck-package-") as temporary:
        stage = Path(temporary) / "package"
        shutil.copytree(root / "npm", stage, ignore=shutil.ignore_patterns("test", "node_modules", "*.tgz"))
        shutil.copy2(root / "LICENSE", stage / "LICENSE")
        (stage / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        (stage / "bin/herduck.cjs").chmod(0o755)
        result = subprocess.run(["npm", "pack", "--ignore-scripts", "--json", "--pack-destination", str(output.resolve())],
                                cwd=stage, check=True, capture_output=True, text=True)
        archive = json.loads(result.stdout)[0]["filename"]
    with (output / archive).open("rb") as handle:
        checksums.append(f"{hashlib.file_digest(handle, 'sha256').hexdigest()}  {archive}\n")
    (output / "SHA256SUMS").write_text("".join(checksums))
    print(output / archive)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assets", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    package_release(Path(__file__).resolve().parent.parent, args.assets, args.output)
