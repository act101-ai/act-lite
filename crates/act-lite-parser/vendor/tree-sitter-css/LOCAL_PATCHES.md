# tree-sitter-css local patches

Vendored at `crates/act-lite-parser/vendor/tree-sitter-css` (path dependency
`tree-sitter-css 0.25.0` in act-lite-parser; upstream
tree-sitter/tree-sitter-css, MIT — Max Brunsfeld / Amaan Qureshi).

Initial state: PRISTINE — a `tree-sitter generate` from the shipped
`grammar.js` (tree-sitter CLI 0.25.10) reproduced `src/parser.c` and
`src/node-types.json` byte-identically (verified 2026-10-04 before the
first local patch below).

## 2026-10-04 — grammar.js edit

The An+B argument rules accepted only `-?\d*n\s*\+\s*\d+` — no minus-B
(`2n-1`), no whitespace around the sign, no plus-signed A (`+2n+1`) —
and `:nth-of-type`/`:nth-last-of-type` had no An+B rule at all (their
arguments fell to the generic value list, so `2n-1` parsed as TWO
values). Per css-syntax-3 §6 (from the CSSWG spec source): whitespace
is permitted on either side of the `+`/`-` joining the An and B parts
(`3n - 2`, `-n+ 6` are valid; `3 n`, `+ 2n`, `+ 2` are not).

- `_nth_functional_notation` regex extended to
  `/[+-]?(\d+)?n\s*([+-]\s*\d+)?/` — whitespace only around the
  joining sign, never inside the An part.
- New `_nth_of_type_pseudo_class_selector` +
  `pseudo_class_nth_of_type_arguments` for `:nth-of-type`/
  `:nth-last-of-type` — the same argument set MINUS the `of S` filter,
  which selectors-4 reserves for `:nth-child`/`:nth-last-child` (the
  of-type variants must refuse it).
- The shared argument choice is factored into a hidden `_nth_argument`
  rule; `pseudo_class_nth_child_arguments` keeps its rule identity and
  its `prec(-1)`.

After the edit: `tree-sitter generate` rc=0. Node-types diff vs the
pristine tree: NONE — no added/removed kinds, no per-type structure
changes (the new rules are hidden or aliased to the existing
`arguments` kind).

## 2026-10-04 — An+B tokens split, casing added

The single-token An+B was case-sensitive (CSS
keywords and `n` are case-insensitive — `2N+1`/`ODD` rejected) and
rejected comments between the An part and the joined B part
(`2n/**/+1`), which CSS tokenization allows. The notation is now TWO
tokens — `_anb_a_part` (`/[+-]?(\d+)?[nN]/`, whitespace inside it
still invalid: `3 n`, `+ 2n` stay refused) and an optional
`_anb_b_part` (`/[+-]\s*\d+/`) — so extras (whitespace, comments) may
sit between them; the sign never stacks (`3n + -6`, the spec's
invalid example, stays an error). `even`/`odd` are case-insensitive
regex tokens now. `plain_value` in node-types gains `"fields": {}`
(the aliased seq is a non-terminal); no kinds added or removed.

## 2026-10-04 — no grammar change

`3n+/**/1` (comment INSIDE the sign+B token):
the spec's three-token production allows comments between each token,
but a three-token split must still reject the stacked-sign forms
structurally. Base-broken too (pre-existing class). No grammar change
in this round.
