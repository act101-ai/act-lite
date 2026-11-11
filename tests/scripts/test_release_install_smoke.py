"""The tagged release run must exercise both published installers."""

from pathlib import Path
import unittest


WORKFLOW = Path(__file__).resolve().parents[2] / ".github/workflows/release.yml"


class ReleaseInstallSmokeTests(unittest.TestCase):
    def test_post_release_job_runs_both_installers(self):
        workflow = WORKFLOW.read_text()
        job = workflow.split("  install-smoke:\n", 1)[1]
        self.assertIn("needs: release", job)
        self.assertIn("ubuntu-latest", job)
        self.assertIn("windows-latest", job)
        self.assertIn("ACT_LITE_VERSION: ${{ github.ref_name }}", job)
        self.assertIn("ACT_LITE_INSTALL_DIR", job)
        self.assertIn("sh install.sh", job)
        self.assertIn("./install.ps1", job)
        self.assertIn("--version", job)


if __name__ == "__main__":
    unittest.main()
