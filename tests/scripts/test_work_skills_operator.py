"""A plan has no operator questions.

Planning decides, each premise and live-check outcome has its action in the
node's contract, and execution takes the recommended option for an open
choice, logs it and continues. The texts that stopped a run to ask the
operator stay removed.
"""

from pathlib import Path
import unittest


SKILLS = Path(__file__).resolve().parents[2] / "plugin/skills"


def read(rel):
    return (SKILLS / rel).read_text(encoding="utf-8")


def flat(text):
    return " ".join(text.split()).lower()


REQUIRED = {
    "plan-work/SKILL.md": (
        "**A plan has no operator questions.**",
        "**No operator questions.**",
        "Questions are asked during planning only",
    ),
    "execute-work/SKILL.md": (
        "**Execution asks the operator nothing.**",
        "### Open choices",
        "Take the recommended option",
        "This notice governs adding a rule.",
    ),
    "work-graph/references/graph-format.md": (
        "**Premises:**",
        "No field leaves a decision to the operator during execution.",
    ),
    "work-graph/references/node-template.md": ("**Premises:**",),
}

REMOVED = (
    "several matches: ask the operator",
    "ask the operator first",
    "created only with the operator's consent",
    "when the operator agrees to keep it",
    "the operator declines the checkout",
    "conflict you cannot resolve from the code goes to the operator",
    "ask about each unclear item",
    "when the project declares none, ask which",
    "when two of these differ, ask",
    "stops and asks the operator",
    "is an operator question",
    "design choices the plan leaves open",
)

CHECKED = (
    "define-work/SKILL.md",
    "plan-work/SKILL.md",
    "execute-work/SKILL.md",
    "deliver-work/SKILL.md",
    "diagnose-defect/SKILL.md",
    "work-graph/references/graph-format.md",
    "work-graph/references/node-template.md",
)


class WorkSkillsOperatorTests(unittest.TestCase):
    def test_each_file_carries_its_part_of_the_rule(self):
        for rel, phrases in REQUIRED.items():
            text = read(rel)
            for phrase in phrases:
                with self.subTest(file=rel, phrase=phrase):
                    self.assertIn(phrase, text)

    def test_texts_that_stopped_a_run_to_ask_stay_removed(self):
        for rel in CHECKED:
            text = flat(read(rel))
            for phrase in REMOVED:
                with self.subTest(file=rel, phrase=phrase):
                    self.assertNotIn(phrase, text)


if __name__ == "__main__":
    unittest.main()
