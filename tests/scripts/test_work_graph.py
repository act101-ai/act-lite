"""Tests for the work-graph companion's bundled work_graph.py (core)."""

from __future__ import annotations

import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[2] / "plugin" / "skills" / "work-graph" / "scripts"
sys.path.insert(0, str(SCRIPTS))

import work_graph as wg  # noqa: E402

HEADER = "| ID | Node | Depends | State | Capability | Refs |\n|----|------|---------|-------|------------|------|\n"
TIER_HEADER = "| ID | Node | Depends | State | Tier | Refs |\n|----|------|---------|-------|-------|------|\n"
LOG_HEADER = "| Date | Event |\n|------|-------|\n"


def row(id_, depends="—", state="TODO", capability="implementation", node=None, refs="—"):
    return f"| {id_} | {node or 'Node ' + id_} | {depends} | {state} | {capability} | {refs} |"


def contract(files="—", consumes="—", produces="—", acceptance="`make test` → pass", omit=()):
    fields = {
        "Spec": "spec.md §1",
        "Files": files,
        "Consumes": consumes,
        "Produces": produces,
        "Acceptance": acceptance,
    }
    body = "".join(f"**{k}:** {v}\n" for k, v in fields.items() if k not in omit)
    return f"# node\n\n{body}\n## Plan\n"


class GraphDir:
    """A temporary graph directory built from rows, contracts and history."""

    def __init__(self, rows, nodes=None, history=None, log=None, run="parallel=3", extra_header="", header=HEADER):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        log = log if log is not None else [("2026-10-21", "Graph created.")]
        text = (
            "# Test Work Graph\n\n<!-- work-graph -->\n\n"
            "**Goal:** test.\n**Authority:** spec.md\n"
            f"**Run:** {run} · worktrees=.worktrees/test/ · disk-per-node=—\n{extra_header}\n"
            "## Nodes\n\n" + header + "\n".join(rows) + "\n\n## Log\n\n" + LOG_HEADER
            + "\n".join(f"| {d} | {e} |" for d, e in log) + "\n"
        )
        self.graph = self.root / "graph.md"
        self.graph.write_text(text, encoding="utf-8")
        (self.root / "nodes").mkdir()
        for id_, body in (nodes or {}).items():
            (self.root / "nodes" / f"{id_}.md").write_text(body, encoding="utf-8")
        if history is not None:
            (self.root / "history.md").write_text(
                "# Test Work Graph — History\n\n## Nodes\n\n" + header + "\n".join(history)
                + "\n\n## Log\n\n" + LOG_HEADER, encoding="utf-8")

    def check(self):
        return wg.check(wg.load(self.graph))

    def close(self):
        self._tmp.cleanup()


class CoreCase(unittest.TestCase):
    def make(self, *args, **kwargs):
        g = GraphDir(*args, **kwargs)
        self.addCleanup(g.close)
        return g

    def assertDefect(self, g, fragment):
        defects = g.check()
        self.assertTrue(any(fragment in d for d in defects), f"{fragment!r} not in {defects}")


class CheckTests(CoreCase):
    def valid(self):
        return self.make(
            [row("N1"), row("N2", "N1"), row("N3", "N1")],
            nodes={
                "N1": contract(files="`src/a.rs`", produces="`parse_config(path) -> Config`"),
                "N2": contract(files="`src/b.rs`", consumes="`parse_config` (N1)", produces="`validate(cfg)`"),
                "N3": contract(files="`src/c.rs`", consumes="`parse_config` (N1)"),
            },
        )

    def test_valid_graph_passes(self):
        g = self.valid()
        self.assertEqual(g.check(), [])
        with redirect_stdout(io.StringIO()):
            self.assertEqual(wg.main(["check", str(g.graph)]), 0)

    def test_cycle_refused(self):
        g = self.make([row("N1", "N2"), row("N2", "N1")], nodes={"N1": contract(), "N2": contract()})
        defects = g.check()
        self.assertTrue(any("cycle" in d and "N1" in d and "N2" in d for d in defects), defects)
        with redirect_stdout(io.StringIO()):
            self.assertEqual(wg.main(["check", str(g.graph)]), 1)

    def test_unknown_edge_refused(self):
        g = self.make([row("N2", "N9")], nodes={"N2": contract()})
        self.assertDefect(g, "N2 depends on unknown node N9")

    def test_missing_contract_field_refused(self):
        g = self.make([row("N2")], nodes={"N2": contract(omit=("Acceptance",))})
        self.assertDefect(g, "N2: contract missing Acceptance")

    def test_acceptance_needs_command_and_expected(self):
        g = self.make([row("N2")], nodes={"N2": contract(acceptance="tests pass")})
        self.assertDefect(g, "N2: Acceptance needs a `command` and → expected output")

    def test_missing_node_file_refused(self):
        g = self.make([row("N3")])
        self.assertDefect(g, "N3: no contract file nodes/N3.md")

    def test_unresolved_consumes_refused(self):
        g = self.make(
            [row("N1"), row("N2", "N1")],
            nodes={"N1": contract(produces="`load(path)`"), "N2": contract(consumes="`parse_config` (N1)")},
        )
        self.assertDefect(g, "N2 consumes parse_config from N1, which does not produce it")

    def test_consumes_from_non_ancestor_refused(self):
        g = self.make(
            [row("N1"), row("N2"), row("N3", "N1")],
            nodes={
                "N1": contract(),
                "N2": contract(produces="`helper()`"),
                "N3": contract(consumes="`helper` (N2)"),
            },
        )
        self.assertDefect(g, "N3 consumes from N2 but does not depend on it")

    def test_transitive_ancestor_accepted(self):
        g = self.make(
            [row("N1"), row("N2", "N1"), row("N3", "N2")],
            nodes={
                "N1": contract(produces="`base()`"),
                "N2": contract(),
                "N3": contract(consumes="`base` (N1)"),
            },
        )
        self.assertEqual(g.check(), [])

    def test_files_overlap_without_path_refused(self):
        g = self.make(
            [row("N2"), row("N3")],
            nodes={"N2": contract(files="`src/a.rs`"), "N3": contract(files="`src/a.rs`, `src/z.rs`")},
        )
        self.assertDefect(g, "N2 and N3 both change src/a.rs")

    def test_files_overlap_with_path_accepted(self):
        g = self.make(
            [row("N2"), row("N3", "N2")],
            nodes={"N2": contract(files="`src/a.rs`"), "N3": contract(files="`src/a.rs`")},
        )
        self.assertEqual(g.check(), [])

    def test_directory_entry_overlaps_files_under_it(self):
        g = self.make(
            [row("N2"), row("N3")],
            nodes={"N2": contract(files="`src/ops/`"), "N3": contract(files="`src/ops/a.rs`")},
        )
        self.assertDefect(g, "N2 and N3 both change src/ops/a.rs")


    def test_directory_entry_does_not_overlap_a_sibling_prefix(self):
        g = self.make(
            [row("N2"), row("N3")],
            nodes={"N2": contract(files="`src/ops/`"), "N3": contract(files="`src/opsx/a.rs`")},
        )
        self.assertEqual(g.check(), [])


    def test_duplicate_id_across_files_refused(self):
        g = self.make([row("N1")], nodes={"N1": contract()}, history=[row("N1", state="DONE")])
        self.assertDefect(g, "duplicate id N1")

    def test_history_open_state_refused(self):
        g = self.make([row("N1")], nodes={"N1": contract()}, history=[row("N0", state="TODO")])
        self.assertDefect(g, "history: N0 is TODO; history holds only DONE or DROPPED")

    def test_bad_state_and_capability_refused(self):
        g = self.make([row("N1", state="WIP", capability="gpt")], nodes={"N1": contract()})
        self.assertDefect(g, "N1: state WIP")
        self.assertDefect(g, "N1: capability gpt is not lookup, mechanical, implementation or judgment")

    def test_model_name_capability_refused_and_judgment_accepted(self):
        bad = self.make([row("N1", capability="opus")], nodes={"N1": contract()})
        self.assertDefect(bad, "N1: capability opus is not lookup, mechanical, implementation or judgment")
        good = self.make([row("N1", capability="judgment")], nodes={"N1": contract()})
        self.assertEqual(good.check(), [])

    def test_capability_graph_validates(self):
        values = ("lookup", "mechanical", "implementation", "judgment")
        g = self.make([row(f"N{i}", capability=v) for i, v in enumerate(values, 1)],
                      nodes={f"N{i}": contract() for i in range(1, 5)})
        self.assertEqual(g.check(), [])
        self.assertEqual([r.capability for r in wg.load(g.graph).rows], list(values))

    def test_tier_graph_validates_with_values_mapped(self):
        g = self.make([row("N1", capability="simple"), row("N2", capability="normal"),
                       row("N3", capability="demanding")],
                      nodes={f"N{i}": contract() for i in range(1, 4)},
                      history=[row("N0", state="DONE", capability="normal")], header=TIER_HEADER)
        self.assertEqual(g.check(), [])
        graph = wg.load(g.graph)
        self.assertEqual([r.capability for r in graph.rows], ["mechanical", "implementation", "judgment"])
        self.assertEqual([r.capability for r in graph.history], ["implementation"])

    def test_tier_graph_unknown_value_refused(self):
        g = self.make([row("N1", capability="opus")], nodes={"N1": contract()}, header=TIER_HEADER)
        self.assertDefect(g, "N1: capability opus is not lookup, mechanical, implementation or judgment")

    def test_tier_value_in_capability_graph_refused(self):
        g = self.make([row("N1", capability="normal")], nodes={"N1": contract()})
        self.assertDefect(g, "N1: capability normal is not lookup, mechanical, implementation or judgment")

    def test_blocked_and_dropped_states_accepted(self):
        g = self.make(
            [row("N1", state="BLOCKED(needs schema)"), row("N2", state="DROPPED(premise gone)")],
            nodes={"N1": contract()},
        )
        self.assertEqual(g.check(), [])

    def test_row_cap_refused(self):
        long = row("N1", node="x" * 600)
        g = self.make([long], nodes={"N1": contract()})
        self.assertDefect(g, "N1: row exceeds 600 characters")

    def test_log_entry_cap_refused(self):
        g = self.make([row("N1")], nodes={"N1": contract()}, log=[("2026-10-21", "y" * 301)])
        self.assertDefect(g, "Log entry 2026-10-21 exceeds 300 characters")

    def test_closed_cap_refused(self):
        rows = [row(f"N{i}", state="DONE") for i in range(1, 27)]
        g = self.make(rows)
        self.assertDefect(g, "26 closed rows; cap is 25")

    def test_log_window_refused(self):
        g = self.make([row("N1")], nodes={"N1": contract()},
                      log=[("2026-10-01", "old"), ("2026-10-21", "new")])
        self.assertDefect(g, "Log entry 2026-10-01 is older than 14 days")

    def test_dependency_done_in_history_counts(self):
        g = self.make([row("N2", "N1")], nodes={"N2": contract()}, history=[row("N1", state="DONE")])
        self.assertEqual(g.check(), [])

    def test_crlf_and_trailing_space_tolerated(self):
        g = self.valid()
        text = g.graph.read_text(encoding="utf-8")
        g.graph.write_bytes(text.replace("|\n", "|  \n").replace("\n", "\r\n").encode())
        self.assertEqual(g.check(), [])

    def test_pipe_inside_backticks_not_split(self):
        g = self.make([row("N1", node="Parse `a | b` input")], nodes={"N1": contract()})
        self.assertEqual(g.check(), [])
        self.assertEqual(wg.load(g.graph).rows[0].node, "Parse `a | b` input")

    def test_conflict_marker_refused(self):
        g = self.valid()
        lines = g.graph.read_text(encoding="utf-8").splitlines()
        lines.insert(5, "<<<<<<< HEAD")
        g.graph.write_text("\n".join(lines) + "\n", encoding="utf-8")
        self.assertDefect(g, "graph.md:6: conflict marker")


class ScheduleTests(CoreCase):
    def test_ready_respects_dependencies(self):
        g = self.make(
            [row("N1", state="DONE"), row("N2", "N1"), row("N3", "N2")],
            nodes={"N2": contract(), "N3": contract()},
        )
        self.assertEqual(wg.schedule(wg.load(g.graph)), [("N2", "ready")])

    def test_in_progress_first_and_counts_toward_parallel(self):
        g = self.make(
            [row("N2"), row("N3"), row("N4", state="IN-PROGRESS")],
            nodes={"N2": contract(), "N3": contract(), "N4": contract()},
            run="parallel=2",
        )
        self.assertEqual(wg.schedule(wg.load(g.graph)), [("N4", "IN-PROGRESS"), ("N2", "ready")])

    def test_parallel_off_is_one(self):
        g = self.make([row("N2"), row("N3")], nodes={"N2": contract(), "N3": contract()}, run="parallel=off")
        self.assertEqual(wg.schedule(wg.load(g.graph)), [("N2", "ready")])

    def test_blocked_node_not_scheduled(self):
        g = self.make([row("N2", state="BLOCKED(x)"), row("N3")], nodes={"N2": contract(), "N3": contract()})
        self.assertEqual(wg.schedule(wg.load(g.graph)), [("N3", "ready")])

    def test_files_overlap_not_scheduled_together(self):
        g = self.make(
            [row("N2", state="IN-PROGRESS"), row("N3"), row("N4")],
            nodes={
                "N2": contract(files="`src/a.rs`"),
                "N3": contract(files="`src/a.rs`"),
                "N4": contract(files="`src/b.rs`"),
            },
        )
        self.assertEqual(wg.schedule(wg.load(g.graph)), [("N2", "IN-PROGRESS"), ("N4", "ready")])

    def test_directory_overlap_not_scheduled_together(self):
        g = self.make(
            [row("N2", state="IN-PROGRESS"), row("N3"), row("N4")],
            nodes={
                "N2": contract(files="`src/ops/`"),
                "N3": contract(files="`src/ops/a.rs`"),
                "N4": contract(files="`src/b.rs`"),
            },
        )
        self.assertEqual(wg.schedule(wg.load(g.graph)), [("N2", "IN-PROGRESS"), ("N4", "ready")])


    def test_ready_command_prints_schedule(self):
        g = self.make([row("N1", state="DONE"), row("N2", "N1")], nodes={"N2": contract()})
        out = io.StringIO()
        with redirect_stdout(out):
            self.assertEqual(wg.main(["ready", str(g.graph)]), 0)
        self.assertEqual(out.getvalue().strip(), "N2 ready")

    def test_status_counts(self):
        g = self.make(
            [row("N1", state="DONE"), row("N3", state="DONE"), row("N2", "N1")],
            nodes={"N2": contract()},
            history=[row("N0", state="DONE")],
        )
        text = wg.status(wg.load(g.graph))
        self.assertIn("TODO 1", text)
        self.assertIn("DONE 2", text)
        self.assertIn("ready: N2", text)
        self.assertIn("history: 1", text)


class LedgerRowsTests(CoreCase):
    """A custom ledger exports normalized JSON rows."""

    def ledger(self, rows, nodes):
        g = self.make([], nodes=nodes)
        path = g.root / "rows.json"
        path.write_text(json.dumps(rows), encoding="utf-8")
        return g, path

    @staticmethod
    def jrow(id_, depends=(), state="TODO", capability="implementation", key="capability"):
        return {"id": id_, "node": f"Node {id_}", "depends": list(depends), "state": state, key: capability, "refs": ""}

    def run_main(self, args):
        out = io.StringIO()
        with redirect_stdout(out):
            code = wg.main(args)
        return code, out.getvalue()

    def test_rows_json_valid_graph_passes(self):
        g, rows = self.ledger(
            [self.jrow("N1"), self.jrow("N2", ["N1"]), self.jrow("N3", ["N1"])],
            {"N1": contract(produces="`base()`"), "N2": contract(consumes="`base` (N1)"), "N3": contract()},
        )
        code, out = self.run_main(["check", str(g.graph), "--rows", str(rows)])
        self.assertEqual(code, 0, out)

    def test_rows_json_capability_loads(self):
        g, rows = self.ledger([self.jrow("N1", capability="lookup"), self.jrow("N2", ["N1"], capability="judgment")],
                              {"N1": contract(), "N2": contract()})
        code, out = self.run_main(["check", str(g.graph), "--rows", str(rows)])
        self.assertEqual(code, 0, out)
        graph = wg.load(g.graph, rows=json.loads(rows.read_text(encoding="utf-8")))
        self.assertEqual([r.capability for r in graph.rows], ["lookup", "judgment"])

    def test_rows_json_legacy_tier_loads_mapped(self):
        g, rows = self.ledger([self.jrow("N1", capability="simple", key="tier"),
                               self.jrow("N2", ["N1"], capability="demanding", key="tier")],
                              {"N1": contract(), "N2": contract()})
        code, out = self.run_main(["check", str(g.graph), "--rows", str(rows)])
        self.assertEqual(code, 0, out)
        graph = wg.load(g.graph, rows=json.loads(rows.read_text(encoding="utf-8")))
        self.assertEqual([r.capability for r in graph.rows], ["mechanical", "judgment"])

    def test_rows_json_missing_capability_refused(self):
        g, rows = self.ledger([{"id": "N1", "depends": [], "state": "TODO"}], {"N1": contract()})
        code, out = self.run_main(["check", str(g.graph), "--rows", str(rows)])
        self.assertEqual(code, 1)
        self.assertIn("row 1: missing capability", out)

    def test_rows_json_cycle_refused(self):
        g, rows = self.ledger([self.jrow("N1", ["N2"]), self.jrow("N2", ["N1"])], {"N1": contract(), "N2": contract()})
        code, out = self.run_main(["check", str(g.graph), "--rows", str(rows)])
        self.assertEqual(code, 1)
        self.assertIn("cycle", out)

    def test_rows_json_accepts_ledger_ids(self):
        g, rows = self.ledger(
            [self.jrow("bd-a1b2"), self.jrow("bd-c3d4", ["bd-a1b2"])],
            {"bd-a1b2": contract(), "bd-c3d4": contract()},
        )
        code, out = self.run_main(["check", str(g.graph), "--rows", str(rows)])
        self.assertEqual(code, 0, out)

    def test_rows_json_ready(self):
        g, rows = self.ledger(
            [self.jrow("N1", state="DONE"), self.jrow("N2", ["N1"]), self.jrow("N3", ["N2"])],
            {"N2": contract(), "N3": contract()},
        )
        code, out = self.run_main(["ready", str(g.graph), "--rows", str(rows)])
        self.assertEqual((code, out.strip()), (0, "N2 ready"))

    def test_rows_json_malformed_refused(self):
        bad = self.jrow("N1")
        del bad["state"]
        g, rows = self.ledger([bad], {"N1": contract()})
        code, out = self.run_main(["check", str(g.graph), "--rows", str(rows)])
        self.assertEqual(code, 1)
        self.assertIn("row 1: missing state", out)

    def test_rows_json_skips_markdown_row_cap(self):
        long = self.jrow("N1")
        long["node"] = "x" * 700
        g, rows = self.ledger([long], {"N1": contract()})
        code, out = self.run_main(["check", str(g.graph), "--rows", str(rows)])
        self.assertEqual(code, 0, out)



class ArchiveTests(CoreCase):
    def closed_graph(self, n_done=27, log=None):
        rows = [row(f"N{i}", state="DONE") for i in range(1, n_done + 1)] + [row("N99")]
        return self.make(rows, nodes={"N99": contract()}, log=log)

    def test_archive_moves_oldest_closed_beyond_cap(self):
        g = self.closed_graph()
        moves = wg.archive(wg.load(g.graph))
        self.assertEqual(len([m for m in moves if "N1 " in m or "N2 " in m]), 2, moves)
        after = wg.load(g.graph)
        self.assertEqual([r.id for r in after.history], ["N1", "N2"])
        self.assertEqual(sum(1 for r in after.rows if r.closed), 25)
        self.assertIn(row("N1", state="DONE"), (g.root / "history.md").read_text(encoding="utf-8"))
        self.assertEqual(after.rows[-1].id, "N99")

    def test_archive_appends_to_existing_history(self):
        g = self.make(
            [row(f"N{i}", state="DONE") for i in range(1, 27)],
            history=[row("N0", state="DONE")],
        )
        wg.archive(wg.load(g.graph))
        self.assertEqual([r.id for r in wg.load(g.graph).history], ["N0", "N1"])

    def test_archive_moves_old_log_entries(self):
        g = self.closed_graph(n_done=1, log=[("2026-09-01", "old"), ("2026-10-21", "new")])
        wg.archive(wg.load(g.graph))
        after = wg.load(g.graph)
        self.assertEqual(after.log, [("2026-10-21", "new")])
        self.assertIn("| 2026-09-01 | old |", (g.root / "history.md").read_text(encoding="utf-8"))

    def test_archive_is_idempotent(self):
        g = self.closed_graph(log=[("2026-09-01", "old"), ("2026-10-21", "new")])
        wg.archive(wg.load(g.graph))
        before = {p.name: p.read_bytes() for p in g.root.glob("*.md")}
        self.assertEqual(wg.archive(wg.load(g.graph)), [])
        self.assertEqual({p.name: p.read_bytes() for p in g.root.glob("*.md")}, before)

    def test_archive_dry_run_writes_nothing(self):
        g = self.closed_graph()
        before = g.graph.read_bytes()
        moves = wg.archive(wg.load(g.graph), dry_run=True)
        self.assertTrue(moves)
        self.assertEqual(g.graph.read_bytes(), before)
        self.assertFalse((g.root / "history.md").exists())

    def test_archive_then_check_passes(self):
        g = self.closed_graph(log=[("2026-09-01", "old"), ("2026-10-21", "new")])
        self.assertTrue(g.check())
        out = io.StringIO()
        with redirect_stdout(out):
            self.assertEqual(wg.main(["archive", str(g.graph)]), 0)
        self.assertEqual(g.check(), [])

    def test_archive_tier_graph_keeps_its_header(self):
        rows = [row(f"N{i}", state="DONE", capability="normal") for i in range(1, 28)] + [row("N99", capability="simple")]
        g = self.make(rows, nodes={"N99": contract()}, header=TIER_HEADER)
        wg.archive(wg.load(g.graph))
        history = (g.root / "history.md").read_text(encoding="utf-8")
        self.assertIn(TIER_HEADER.splitlines()[0], history)
        self.assertIn(row("N1", state="DONE", capability="normal"), history)
        self.assertEqual(g.check(), [])
        self.assertEqual([r.id for r in wg.load(g.graph).history], ["N1", "N2"])


class TemplateTests(unittest.TestCase):
    """The shipped templates, filled in, pass check: template and script agree."""

    @staticmethod
    def block(name):
        text = (SCRIPTS.parent / "references" / name).read_text(encoding="utf-8")
        return text.split("```markdown\n", 1)[1].split("\n```", 1)[0]

    def test_graph_and_node_templates_pass_check(self):
        import re
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            graph = self.block("graph-template.md")
            optional = ("**Next:**", "**Findings:**", "**Ledger:**", "**Workflows:**", "**Hygiene:**")
            graph = "\n".join(line for line in graph.splitlines() if not line.startswith(optional))
            graph = graph.replace("<YYYY-MM-DD>", "2026-10-21")
            graph = re.sub(r"<[^<>]+>", "x", graph)
            (root / "graph.md").write_text(graph + "\n", encoding="utf-8")
            node = self.block("node-template.md").replace("`<name>` (<id of the node that produces it>)", "—")
            node = re.sub(r"<[^<>]+>", "x", node)
            (root / "nodes").mkdir()
            (root / "nodes" / "N1.md").write_text(node + "\n", encoding="utf-8")
            self.assertIn(HEADER.splitlines()[0], graph)
            loaded = wg.load(root / "graph.md")
            self.assertEqual([r.id for r in loaded.rows], ["N1"])
            self.assertEqual(wg.check(loaded), [])


class ReviewFixTests(CoreCase):
    """Findings from the adversarial review (R*, L*), each pinned by a test."""

    def run_main(self, args):
        out = io.StringIO()
        with redirect_stdout(out):
            code = wg.main(args)
        return code, out.getvalue()

    def test_r2_missing_graph_file_refused(self):
        code, out = self.run_main(["check", "/nonexistent/graph.md"])
        self.assertEqual(code, 1)
        self.assertIn("not found", out)

    def test_r2_missing_node_table_refused(self):
        g = self.make([row("N1")], nodes={"N1": contract()})
        g.graph.write_text(g.graph.read_text(encoding="utf-8").replace("## Nodes", "## Nodes table"), encoding="utf-8")
        self.assertDefect(g, "graph.md: no node table")

    def test_r2_wrong_node_table_header_refused(self):
        g = self.make([row("N1")], nodes={"N1": contract()})
        text = g.graph.read_text(encoding="utf-8").replace("| Refs |\n", "| Refs | Notes |\n", 1)
        g.graph.write_text(text, encoding="utf-8")
        self.assertDefect(g, "graph.md: no node table")

    def test_r3_run_separators_and_values(self):
        for run in ("parallel=1; worktrees=.w/", "parallel=1, worktrees=.w/", "parallel=1 · worktrees=.w/",
                    "parallel = 1 · worktrees = .w/"):
            g = self.make([row("N1"), row("N2")], nodes={"N1": contract(), "N2": contract()}, run=run)
            self.assertEqual(wg.load(g.graph).parallel(), 1, run)

    def test_r3_bad_parallel_value_refused(self):
        g = self.make([row("N1")], nodes={"N1": contract()}, run="parallel=two")
        self.assertDefect(g, "Run: parallel=two is not a whole number or off")

    def test_r7_conflict_marker_in_node_file_refused(self):
        g = self.make([row("N1")], nodes={"N1": "<<<<<<< HEAD\n" + contract() + ">>>>>>> b\n"})
        self.assertDefect(g, "nodes/N1.md:1: conflict marker")

    def test_r9_consumes_item_without_id_refused(self):
        g = self.make(
            [row("N1"), row("N2", "N1")],
            nodes={"N1": contract(produces="`load(p)`"), "N2": contract(consumes="`load` (N1), `missing_fn`")},
        )
        self.assertDefect(g, "N2: Consumes item `missing_fn` names no producer (ID)")

    def test_r10_qualified_name_mismatch_refused(self):
        g = self.make(
            [row("N1"), row("N2", "N1")],
            nodes={"N1": contract(produces="`config::load(path) -> Config`"), "N2": contract(consumes="`config::parse` (N1)")},
        )
        self.assertDefect(g, "N2 consumes config::parse from N1, which does not produce it")

    def test_r10_method_name_mismatch_refused(self):
        g = self.make(
            [row("N1"), row("N2", "N1")],
            nodes={"N1": contract(produces="`Config.load()`"), "N2": contract(consumes="`Config.validate` (N1)")},
        )
        self.assertDefect(g, "N2 consumes Config.validate from N1, which does not produce it")

    def test_r10_keyword_signature_and_last_segment_accepted(self):
        g = self.make(
            [row("N1"), row("N2", "N1"), row("N3", "N1")],
            nodes={
                "N1": contract(produces="`fn load(path: &Path) -> Config`, `config::parse(s)`, `MAX: usize`"),
                "N2": contract(consumes="`load` (N1), `config::parse` (N1)"),
                "N3": contract(consumes="`parse` (N1), `MAX` (N1)"),
            },
        )
        self.assertEqual(g.check(), [])

    def test_r15_history_without_log_section(self):
        g = self.make([row(f"N{i}", state="DONE") for i in range(1, 27)])
        (g.root / "history.md").write_text("# H\n\n## Nodes\n\n" + HEADER, encoding="utf-8")
        wg.archive(wg.load(g.graph))
        self.assertEqual([r.id for r in wg.load(g.graph).history], ["N1"])

    def test_r15_old_log_into_history_without_log_section(self):
        g = self.make([row("N1")], nodes={"N1": contract()}, log=[("2026-09-01", "old"), ("2026-10-21", "new")])
        (g.root / "history.md").write_text("# H\n\n## Nodes\n\n" + HEADER, encoding="utf-8")
        wg.archive(wg.load(g.graph))
        self.assertIn("| 2026-09-01 | old |", (g.root / "history.md").read_text(encoding="utf-8"))
        self.assertEqual(g.check(), [])

    def test_r16_archive_keeps_untouched_lines_and_line_endings(self):
        g = self.make([row(f"N{i}", state="DONE") for i in range(1, 27)] + [row("N99")], nodes={"N99": contract()})
        text = g.graph.read_text(encoding="utf-8").replace("**Goal:** test.", "**Goal:** test.  ")
        g.graph.write_bytes(text.replace("\n", "\r\n").encode())
        wg.archive(wg.load(g.graph))
        raw = g.graph.read_bytes()
        self.assertNotIn(b"\n", raw.replace(b"\r\n", b""))
        self.assertIn(b"**Goal:** test.  \r\n", raw)

    def test_r17_status_ready_set_is_not_the_schedule(self):
        rows = [row(f"N{i}") for i in range(1, 5)]
        g = self.make(rows, nodes={f"N{i}": contract() for i in range(1, 5)}, run="parallel=1")
        text = wg.status(wg.load(g.graph))
        self.assertIn("ready: N1, N2, N3, N4", text)
        self.assertIn("schedule: N1", text)

    def test_r18_double_backtick_span(self):
        self.assertEqual(len(wg.split_cells("| N1 | Parse ``a`b`` spans | — | TODO | normal | — |")), 6)

    def test_r21_status_reports_stuck_nodes(self):
        g = self.make([row("N1", state="DROPPED(gone)"), row("N2", "N1"), row("N3", state="BLOCKED(x)")],
                      nodes={"N2": contract(), "N3": contract()})
        text = wg.status(wg.load(g.graph))
        self.assertIn("stuck: N2 (depends on DROPPED N1), N3 (BLOCKED)", text)

    def test_l2_archive_with_rows_moves_only_log(self):
        g = self.make([], log=[("2026-09-01", "old"), ("2026-10-21", "new")])
        rows = g.root / "rows.json"
        rows.write_text(json.dumps([{"id": "N1", "depends": [], "state": "TODO", "capability": "implementation"}]), encoding="utf-8")
        (g.root / "nodes" / "N1.md").write_text(contract(), encoding="utf-8")
        code, out = self.run_main(["check", str(g.graph), "--rows", str(rows)])
        self.assertEqual(code, 1)
        code, out = self.run_main(["archive", str(g.graph), "--rows", str(rows)])
        self.assertEqual(code, 0, out)
        self.assertIn("Log 2026-09-01", out)
        code, out = self.run_main(["check", str(g.graph), "--rows", str(rows)])
        self.assertEqual(code, 0, out)

    def test_l7_core_has_no_open_cap(self):
        rows = [row(f"N{i}") for i in range(1, 31)]
        g = self.make(rows, nodes={f"N{i}": contract() for i in range(1, 31)})
        saved, wg.EXT = wg.EXT, None
        try:
            self.assertEqual(g.check(), [])
        finally:
            wg.EXT = saved


if __name__ == "__main__":
    unittest.main()
