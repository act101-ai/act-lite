"""Release checker fixtures use the asset names emitted by release.yml."""

import hashlib
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
import zipfile


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts/check_release.sh"
TAG = "v1.2.2"
TARGETS = (
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
)


class ReleaseCheckerTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        base = Path(self.tmp.name)
        self.assets = base / "assets"
        self.assets.mkdir()
        fake_bin = base / "bin"
        fake_bin.mkdir()
        gh = fake_bin / "gh"
        gh.write_text(
            "#!/bin/sh\n"
            "[ \"$1 $2\" = 'release download' ] || exit 2\n"
            "while [ \"$#\" -gt 0 ]; do\n"
            "  if [ \"$1\" = --dir ]; then dest=$2; break; fi\n"
            "  shift\n"
            "done\n"
            "[ -n \"${dest:-}\" ] || exit 2\n"
            "cp \"$FAKE_RELEASE_DIR\"/* \"$dest/\"\n"
        )
        gh.chmod(0o755)
        self.env = os.environ.copy()
        self.env["PATH"] = f"{fake_bin}{os.pathsep}{self.env['PATH']}"
        self.env["FAKE_RELEASE_DIR"] = str(self.assets)

        binary = b"#!/bin/sh\nprintf 'act-lite 1.2.2\\n'\n"
        for target in TARGETS:
            name = f"act-lite-{TAG}-{target}"
            if target.endswith("windows-msvc"):
                archive = self.assets / f"{name}.zip"
                with zipfile.ZipFile(archive, "w") as zf:
                    zf.writestr(f"{name}/act-lite.exe", binary)
            else:
                archive = self.assets / f"{name}.tar.gz"
                with tarfile.open(archive, "w:gz") as tf:
                    info = tarfile.TarInfo(f"{name}/act-lite")
                    info.mode = 0o755
                    info.size = len(binary)
                    tf.addfile(info, io.BytesIO(binary))
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            (self.assets / f"{archive.name}.sha256").write_text(
                f"{digest}  {archive.name}\n"
            )

    def check(self):
        return subprocess.run(
            ["bash", str(CHECKER), TAG], cwd=ROOT, env=self.env,
            capture_output=True, text=True,
        )

    def test_complete_release_passes(self):
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("act-lite 1.2.2", result.stdout)

    def test_missing_archive_fails(self):
        (self.assets / f"act-lite-{TAG}-{TARGETS[0]}.tar.gz").unlink()
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing", result.stderr)

    def test_missing_checksum_fails(self):
        (self.assets / f"act-lite-{TAG}-{TARGETS[1]}.tar.gz.sha256").unlink()
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing", result.stderr)

    def test_bad_checksum_fails(self):
        archive = f"act-lite-{TAG}-{TARGETS[2]}.tar.gz"
        (self.assets / f"{archive}.sha256").write_text(f"{'0' * 64}  {archive}\n")
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("checksum", result.stderr)


if __name__ == "__main__":
    unittest.main()
