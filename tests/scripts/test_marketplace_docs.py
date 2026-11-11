"""Installation docs agree with the published act101 marketplace plugin."""

from pathlib import Path
import unittest


README = Path(__file__).resolve().parents[2] / "README.md"


class MarketplaceDocsTests(unittest.TestCase):
    def test_three_harness_install_paths(self):
        text = README.read_text()
        self.assertIn("claude plugin marketplace add act101-ai/act101", text)
        self.assertIn("claude plugin install act-lite@act101-marketplace", text)
        self.assertIn("codex plugin marketplace add act101-ai/act101", text)
        self.assertIn("codex plugin add act-lite@act101-marketplace", text)
        self.assertIn("plugin-act-lite/skills", text)
        self.assertIn('"act-lite", "mcp", "serve"', text)
        self.assertNotIn("/plugin marketplace add act101-ai/act-lite", text)
        opencode = text.split("For OpenCode,", 1)[1].split("## License", 1)[0]
        self.assertIn("~/.config/opencode/act-lite/skills", opencode)
        self.assertIn("OPENCODE_CONFIG_DIR=", opencode)
        self.assertNotIn('"skills":', opencode)
        self.assertNotIn("cp -R act101-marketplace/plugin-act-lite/skills/. ~/.config/opencode/skills/", opencode)


if __name__ == "__main__":
    unittest.main()
