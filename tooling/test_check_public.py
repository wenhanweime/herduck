from pathlib import Path
import subprocess
import tempfile
import unittest

from check_public import check_public, inspect_text


class PublicPolicyTests(unittest.TestCase):
    def test_detects_home_paths_on_supported_platforms_and_in_session_ids(self):
        for value in (
            "/Users/private-account/projects/app",
            "/home/private-account/projects/app",
            r"C:\Users\private-account\projects\app",
            r"C:\\Users\\private-account\\projects\\app",
            "%2FUsers%2Fprivate-account%2Fprojects",
            "%252FUsers%252Fprivate-account%252Fprojects",
        ):
            with self.subTest(value=value):
                self.assertEqual(
                    inspect_text("src/sample.rs", value),
                    [("src/sample.rs", 1, "personal-home-path")],
                )

    def test_allows_neutral_examples_upstream_ancestry_and_public_repository_urls(self):
        text = "\n".join((
            "/Users/example/projects/app", "/home/test/project", "/home/a b/.ssh/config",
            "/Users/example <INSTRUCTIONS>", r"C:\Users\herdr\project",
            "/home/linuxbrew/.linuxbrew/bin/herdr", "arrows/home/end",
            "https://github.com/wenhanweime/herduck", "HERDR_ENV", "herdr-agent-state.ts",
        ))
        self.assertEqual(inspect_text("src/sample.rs", text), [])

    def test_detects_claude_project_paths_without_decoding_arbitrary_hyphens(self):
        for value in (
            "~/.claude/projects/-Users-private-account-Workspace/session.jsonl",
            "~/.claude/projects/-home-private-account-project/session.jsonl",
        ):
            self.assertEqual(inspect_text("docs/sample.md", value), [
                ("docs/sample.md", 1, "claude-project-home-path"),
            ])
        for value in (
            "~/.claude/projects/-Users-example-Workspace/session.jsonl",
            "~/.claude/projects/-home-test-project/session.jsonl",
            "unrelated-Users-project-name",
        ):
            self.assertEqual(inspect_text("docs/sample.md", value), [])

    def test_private_defaults_report_locations_without_echoing_contents(self):
        text = "\n".join((
            "paseo-multica-agent-private", "ork-direct-accept.private",
            "NewAPIConn/private-model", "wenhanwei-private",
        ))
        violations = inspect_text("src/sample.rs", text)
        self.assertEqual([item[1] for item in violations], [1, 2, 3, 4])
        self.assertNotIn("private-model", str(violations))

    def test_scans_root_contribution_and_security_guides(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "--quiet"], cwd=root, check=True)
            for name in ("CONTRIBUTING.md", "SECURITY.md"):
                (root / name).write_text("/Users/private-account/project")
            checked, violations = check_public(root)
            self.assertEqual(checked, 2)
            self.assertEqual({item[0] for item in violations}, {"CONTRIBUTING.md", "SECURITY.md"})

    def test_scans_tracked_and_new_public_files_but_not_ignored_personal_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "--quiet"], cwd=root, check=True)
            (root / "src").mkdir()
            (root / "docs").mkdir()
            (root / "vendor").mkdir()
            (root / ".local").mkdir()
            (root / ".gitignore").write_text(".local/\ndocs/legacy.md\n")
            (root / "src/tracked.rs").write_text('let cwd = "/Users/private-account/project";')
            (root / "docs/legacy.md").write_text("NewAPIConn/model")
            subprocess.run(["git", "add", "src/tracked.rs"], cwd=root, check=True)
            # Adding an ignore rule never makes an already tracked leak acceptable.
            subprocess.run(["git", "add", "-f", "docs/legacy.md"], cwd=root, check=True)
            (root / "src/new.rs").write_text("paseo-topics-agent-private")
            (root / ".local/config.toml").write_text("NewAPIConn/model")
            (root / "vendor/upstream.rs").write_text("/home/private-account/upstream")

            checked, violations = check_public(root)
            self.assertEqual(checked, 3)
            self.assertEqual({item[0] for item in violations}, {
                "src/tracked.rs", "src/new.rs", "docs/legacy.md",
            })

            (root / "src/tracked.rs").unlink()
            (root / "src/new.rs").write_text("// portable source")
            (root / "docs/legacy.md").write_text("Public design notes")
            self.assertEqual(check_public(root), (2, []))


if __name__ == "__main__":
    unittest.main()
