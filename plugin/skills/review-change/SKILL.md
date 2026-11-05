---
name: review-change
description: "Reviews a change for code health: structural problems in the changed functions and duplication that touches the change. Use when the operator asks to review a diff, branch, commit range or pull request for complexity, size, nesting, parameter lists or duplicated code."
---

# Review Change

This skill reviews one change for code health with the act-lite MCP tools.
It reports structural problems in the functions the change touches and the
duplication the change takes part in. It ends in a report and never edits a
file.

## Adapt

An operator instruction wins over every default below: the base to compare
against, the files to review, whether tests are reviewed.

The tools are the `act-lite` MCP server's. The same results come from the
`act-lite analyze` command (`act-lite analyze patterns …`), so a session
without the MCP server runs the commands instead.

act-lite does not run `git`; you do. Run only the read-only `git` commands
below. Never stash, reset, check out or commit.

## Inputs

- **Base:** a git ref. Default: the merge base of `HEAD` with the default
  branch (`git merge-base HEAD origin/HEAD`, else `main`, else `master`).
  The change is the working tree against the base.
- **Or a file list:** the files to review. Every line of each listed file
  counts as changed, and step 1 is skipped.
- **`include_tests`:** `false`, unless the operator asks to review tests too.
  Pass it to both tools only then.

## Procedure

1. **Changed files and changed lines.** Skip this step for a file list.
   - Files: `git diff --name-only --diff-filter=AMR <base>`, plus the
     untracked files from `git ls-files --others --exclude-standard`.
     Every line of an untracked file counts as changed.
   - Lines: `git diff -U0 --diff-filter=AMR <base>`. Each hunk header
     `@@ -a,b +c,d @@` marks the new-side lines `c` to `c + d - 1` as
     changed. A hunk that only deletes lines (`d` = 0) marks the new-side
     line `c` at the deletion point as changed, so the function around a
     deletion is reviewed.
   - Paths are relative to the repository root, which is the workspace of
     the act-lite server.

2. **Nothing to review, or nothing in scope.**
   - If no file changed, write the report with "No file changed against
     `<base>`." and call no tool.
   - If a tool call returns the error that starts with `no files to analyze
     in the requested scope` (for example every changed file is a test),
     quote the message in the report's Not analyzed section, and still run
     the other step.

3. **Structural problems.** Call `analyze_patterns` with
   `paths` = the changed files and `limit` = 1000.
   - Keep each finding whose `range` (`"L:C-L:C"`, lines inclusive) overlaps
     a changed line of its `file`.
   - If the result has `truncated: true`, split the file list in two halves
     and call once per half, again while a result is truncated and its list
     has more than one file. Concatenate the findings in file order. A single
     file that is still truncated contributes its first 1000 findings, and
     the Limits section gives its `total`.

4. **Duplication.** Call `analyze_clones` with `paths` = `[]` (the whole
   workspace), `focus` = the changed files and `limit` = 1000.
   - Keep each clone class with a member whose `file` is a changed file and
     whose `range` overlaps a changed line of that file.
   - If the result has `truncated: true`, split `focus` in two halves and call
     once per half, again while a result is truncated and its list has more
     than one file. A single file that is still truncated contributes its
     first 1000 classes, and the Limits section gives its `total`. Each call
     analyzes the whole workspace, so a split is the rare case.

5. **Report.** Write the report below from the kept findings and classes and
   from each result's `status`, `complete`, `coverage`, `excluded`,
   `incomplete` and `details`. Write nothing else, and change no file.

## Report

```
## Code health review: <base>..<working tree>
Scope: <n> changed files; patterns <status>, complete <bool>; clones <status>, complete <bool>.
### Structural problems in changed functions
| Location | Function | Rule | Value | Threshold |
### Duplication that touches the change
- <kind> clone, <tokens> tokens, <members_total> members: <file>:<range> (variant <v>), …
### Not analyzed
Unsupported languages, excluded counts (tests, generated, vendored, ignored), skipped files, parse failures.
### Limits
A clone class shows that code has the same syntax tree, apart from formatting, comments and, for renamed clones, identifier names and literal values. It does not show that the members behave the same, that they can share one function, or that extracting them is safe. Findings are measurements against fixed thresholds (<details.thresholds>).
```

- **Structural problems:** one row per kept finding, in result order.
  Location is `<file>:<range>`; Function is `function`, or `(anonymous)` for
  `null`; Rule, Value and Threshold are `rule`, `value` and `threshold`.
  With no kept finding, write "None in the changed functions."
- **Duplication:** one line per kept class, in result order, with its
  `members` (each `<file>:<range> (variant <variant>)`); when
  `members_total` is more than the members listed, add "and <n> more".
  With no kept class, write "None that touches the change."
- **Not analyzed:** from each result, the `coverage.languages` entries with
  `supported: false`, the `excluded.counts`, the `incomplete.skipped_files`
  and the `incomplete.parse_failures`, and any empty-scope message from step
  2. A result with `complete: false` can miss findings in that evidence; say
  so.
- **Limits:** the sentence above, the thresholds from the `analyze_patterns`
  result's `details.thresholds`, and, for each split that stayed truncated,
  the file and its `total`.
