"""The plugin launcher starts act-lite with no prior install of the binary."""

import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]
PLUGIN = ROOT / "plugin"
LAUNCHER = PLUGIN / "bin/act-lite-mcp"
VERSION = json.loads((PLUGIN / ".claude-plugin/plugin.json").read_text())["version"]
TAG = f"v{VERSION}"
TARGET = "aarch64-apple-darwin"
NAME = f"act-lite-{TAG}-{TARGET}"


def fake_binary(label):
    """A stand-in act-lite: prints its version, or its label and arguments."""
    return (
        "#!/bin/sh\n"
        f'if [ "$1" = --version ]; then echo "act-lite {VERSION}"; exit 0; fi\n'
        f'echo "{label} $*"\n'
    ).encode()


class PluginLauncherTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        base = Path(self.tmp.name)
        self.release = base / "release"
        (self.release / TAG).mkdir(parents=True)
        self.data = base / "data"
        self.empty_path = base / "path"
        self.empty_path.mkdir()
        self.publish(fake_binary("downloaded"))

    def publish(self, content, checksum=None):
        buf = io.BytesIO()
        with tarfile.open(fileobj=buf, mode="w:gz") as tar:
            info = tarfile.TarInfo(f"{NAME}/act-lite")
            info.size = len(content)
            info.mode = 0o755
            tar.addfile(info, io.BytesIO(content))
        archive = buf.getvalue()
        digest = checksum or hashlib.sha256(archive).hexdigest()
        (self.release / TAG / f"{NAME}.tar.gz").write_bytes(archive)
        (self.release / TAG / f"{NAME}.tar.gz.sha256").write_text(f"{digest}  {NAME}.tar.gz\n")

    def run_launcher(self, path=None, base_url=None):
        env = {
            "HOME": self.tmp.name,
            "PATH": f"{path or self.empty_path}:/usr/bin:/bin",
            "CLAUDE_PLUGIN_DATA": str(self.data),
            "ACT_LITE_BASE_URL": base_url or f"file://{self.release}",
            "ACT_LITE_UNAME_S": "Darwin",
            "ACT_LITE_UNAME_M": "arm64",
        }
        return subprocess.run(
            [str(LAUNCHER), "mcp", "serve"], env=env, capture_output=True, text=True
        )

    def test_mcp_config_runs_the_launcher(self):
        config = json.loads((PLUGIN / ".mcp.json").read_text())["mcpServers"]["act-lite"]
        self.assertEqual(config["command"], "${CLAUDE_PLUGIN_ROOT}/bin/act-lite-mcp")
        self.assertEqual(config["args"], ["mcp", "serve"])
        self.assertTrue(os.access(LAUNCHER, os.X_OK))

    def test_first_start_downloads_verifies_and_runs(self):
        result = self.run_launcher()
        self.assertEqual(result.returncode, 0, result.stderr)
        # stdout is the MCP channel: only the server writes to it
        self.assertEqual(result.stdout, "downloaded mcp serve\n")
        self.assertTrue((self.data / TAG / "act-lite").is_file())
        self.assertEqual(sorted(p.name for p in self.data.iterdir()), [TAG])

    def test_later_start_uses_the_kept_binary_without_network(self):
        self.assertEqual(self.run_launcher().returncode, 0)
        result = self.run_launcher(base_url=f"file://{self.tmp.name}/missing")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "downloaded mcp serve\n")

    def test_checksum_mismatch_installs_nothing(self):
        self.publish(fake_binary("downloaded"), checksum="0" * 64)
        result = self.run_launcher()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")
        self.assertIn("checksum mismatch", result.stderr)
        self.assertFalse((self.data / TAG / "act-lite").exists())

    def test_matching_act_lite_on_path_is_used(self):
        on_path = Path(self.tmp.name) / "onpath"
        on_path.mkdir()
        (on_path / "act-lite").write_bytes(fake_binary("from-path"))
        (on_path / "act-lite").chmod(0o755)
        result = self.run_launcher(path=on_path)
        self.assertEqual(result.stdout, "from-path mcp serve\n")
        self.assertFalse(self.data.exists())

    def test_other_version_on_path_is_ignored(self):
        on_path = Path(self.tmp.name) / "onpath"
        on_path.mkdir()
        (on_path / "act-lite").write_text("#!/bin/sh\necho 'act-lite 0.0.1'\n")
        (on_path / "act-lite").chmod(0o755)
        result = self.run_launcher(path=on_path)
        self.assertEqual(result.stdout, "downloaded mcp serve\n")

    def test_older_kept_versions_are_removed(self):
        old = self.data / "v0.0.1"
        old.mkdir(parents=True)
        (old / "act-lite").write_text("old")
        self.assertEqual(self.run_launcher().returncode, 0)
        self.assertFalse(old.exists())

    def test_stale_downloads_are_removed_and_fresh_ones_kept(self):
        self.assertEqual(self.run_launcher().returncode, 0)
        stale = self.data / ".download.stale"
        fresh = self.data / ".download.fresh"
        for d in (stale, fresh):
            d.mkdir()
            (d / "partial.tar.gz").write_bytes(b"x")
        hour_ago = time.time() - 3600
        os.utime(stale, (hour_ago, hour_ago))
        result = self.run_launcher()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(stale.exists())
        self.assertTrue(fresh.exists())

    def test_unsupported_platform_fails_on_stderr(self):
        env_result = subprocess.run(
            [str(LAUNCHER), "mcp", "serve"],
            env={
                "HOME": self.tmp.name,
                "PATH": f"{self.empty_path}:/usr/bin:/bin",
                "CLAUDE_PLUGIN_DATA": str(self.data),
                "ACT_LITE_BASE_URL": f"file://{self.release}",
                "ACT_LITE_UNAME_S": "FreeBSD",
                "ACT_LITE_UNAME_M": "amd64",
            },
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(env_result.returncode, 0)
        self.assertEqual(env_result.stdout, "")
        self.assertIn("unsupported platform", env_result.stderr)


if __name__ == "__main__":
    unittest.main()
