
; Rule sets (CSS rules) — see the css file for why `(selectors)` and why the
; name keeps its sigils.
(rule_set
  (selectors) @name) @definition

(declaration
  (property_name) @name) @definition

; Mixin declarations (@mixin)
(mixin_statement
  name: (identifier) @name) @definition

; Function declarations (@function)
(function_statement
  name: (identifier) @name) @definition

; Use statements (@use). tree-sitter-scss exposes no `string_content` child,
; so the name keeps its quotes (`"sass:math"`).
(use_statement
  (string_value) @name) @definition

; Keyframes (@keyframes)
(keyframes_statement
  (keyframes_name) @name) @definition

