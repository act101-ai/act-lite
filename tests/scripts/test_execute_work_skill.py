"""Checks for the execute-work handoff contract."""

from pathlib import Path
import unittest


SKILL = Path(__file__).resolve().parents[2] / "plugin/skills/execute-work/SKILL.md"


class ExecuteWorkSkillTests(unittest.TestCase):
    def test_controller_chores_do_not_claim_a_node_unit(self):
        text = SKILL.read_text()
        self.assertIn("Graph edits and archive operations are controller chores", text)
        self.assertIn("never use a node `Unit:`", text)
        self.assertIn("`Unit: <program>`", text)

    def test_check_runs_alone_without_pipes_or_writes(self):
        text = SKILL.read_text()
        resume = text.split("### 5.1 Resume", 1)[1].split("### 5.2 One node", 1)[0]
        self.assertIn("with at most trailing `2>&1`", " ".join(resume.split()))
        self.assertIn("Run a node's check alone as one command", text)
        self.assertIn("Do not pipe it through `tee` or `cat`", text)
        self.assertIn("chain it with a write", text)
        self.assertIn("A pipe anywhere is no match", text)
        self.assertIn("BIONIC_*", text)
        self.assertIn("RUST_*", text)
        self.assertIn("CARGO_*", text)
        self.assertIn("CLICOLOR*", text)
        self.assertIn('`; echo "exit=$?"`', text)

    def test_compact_once_before_each_row_when_available(self):
        text = SKILL.read_text()
        row_loop = text.split("### 5.2 One node", 1)[1].split("### 5.3", 1)[0]
        self.assertIn("once before starting each row", row_loop)
        self.assertIn("call `compact` without parameters", row_loop)
        self.assertIn("reply text as the P7 record (`done`)", row_loop)


if __name__ == "__main__":
    unittest.main()
