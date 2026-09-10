#!/usr/bin/env python3
"""Reject machine-specific defaults and fixtures in files intended for publication."""

from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote


PUBLIC_PATHS = (
    "src", "tests", "docs", "tooling", ".github", "README.md", "Cargo.toml",
    "build.rs", "justfile", "rust-toolchain.toml", "CONTRIBUTING.md", "SECURITY.md",
    "npm", "README.zh-CN.md",
)
# These two files contain the policy and deliberate failing samples, not product configuration.
POLICY_FILES = {"tooling/check_public.py", "tooling/test_check_public.py"}
EXAMPLE_USERS = {"example", "test", "user", "me", "someone", "a b", "linuxbrew", "herdr"}
HOME_PATH = re.compile(
    r"(?<![\w])(?:/(?:Users|home)/|[A-Za-z]:(?:/{1,2}|\\{1,2})Users(?:/{1,2}|\\{1,2}))"
    r"([^/\\\r\n\"'`<>]+)",
    re.IGNORECASE,
)
CLAUDE_PROJECT_PATH = re.compile(
    r"\.claude[/\\]+projects[/\\]+-(?:Users|home)-([^- /\\\r\n\"'`<>]+)",
    re.IGNORECASE,
)
RULES = (
    ("private-runner-or-project", re.compile(r"\b(?:paseo(?:[-_][\w.-]*)?|ork-direct-accept)\b", re.I)),
    ("machine-provider-alias", re.compile(r"\bNewAPIConn\b", re.I)),
    ("personal-identifier", re.compile(r"\bwenhanwei(?!me/herduck\b)[\w-]*\b", re.I)),
)


def inspect_text(path, text):
    violations = []
    for number, line in enumerate(text.splitlines(), 1):
        # Session IDs can contain percent-encoded home paths, including nested URI encoding.
        decoded = unquote(unquote(line))
        if any(match[1].strip().lower() not in EXAMPLE_USERS for match in HOME_PATH.finditer(decoded)):
            violations.append((path, number, "personal-home-path"))
        if any(match[1].lower() not in EXAMPLE_USERS for match in CLAUDE_PROJECT_PATH.finditer(decoded)):
            violations.append((path, number, "claude-project-home-path"))
        for rule, pattern in RULES:
            if pattern.search(decoded):
                violations.append((path, number, rule))
    return violations


def check_public(root):
    result = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", *PUBLIC_PATHS],
        cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True,
    )
    paths = sorted(set(result.stdout.decode("utf-8").split("\0")) - {""})
    violations = []
    checked = 0
    for name in paths:
        if name in POLICY_FILES:
            continue
        path = root / name
        if path.is_symlink():
            # Git publishes the link itself. Never read a link's target outside the checkout.
            content = str(path.readlink())
        elif path.is_file():
            data = path.read_bytes()
            if b"\0" in data:
                continue
            content = data.decode("utf-8")
        else:
            continue  # A tracked file may have been deleted in the current diff.
        checked += 1
        violations.extend(inspect_text(name, content))
    return checked, violations


def main():
    try:
        checked, violations = check_public(Path(__file__).resolve().parent.parent)
    except (OSError, UnicodeError, subprocess.CalledProcessError):
        print("Public check could not read the Git file list or source files.", file=sys.stderr)
        return 2
    for path, line, rule in violations:
        # Do not echo potentially private file contents into CI output.
        print(f"{path}:{line}: {rule}")
    if violations:
        print(f"Public check failed: {len(violations)} violation(s). Move personal settings to config.toml.")
        return 1
    print(f"Public check passed: {checked} source, test, documentation, and tooling files.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
