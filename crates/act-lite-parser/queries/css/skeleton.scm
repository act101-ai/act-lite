
; Rule sets (CSS rules).
;
; `(selectors)` is present on EVERY `rule_set`, where `symbols.scm`'s finer
; `class_selector`/`id_selector`/`tag_name` alternation leaves attribute,
; pseudo-class, universal and descendant selectors unmatched — a different
; query answering a different question (one symbol per selector piece vs one
; declaration per block). The name keeps its sigils (`.btn`, `#main`): the
; sigil is what distinguishes a class from an id of the same name.
(rule_set
  (selectors) @name) @definition

; Media queries — the first named child is the media query itself.
(media_statement
  . (_) @name) @definition

; Keyframe animations
(keyframes_statement
  (keyframes_name) @name) @definition

; Supports queries — the first named child is the feature query.
(supports_statement
  . (_) @name) @definition

; Generic at-rules (@font-face, @page, @layer, etc.).
; The generic at-rule's identity *is* its at-keyword.
(at_rule
  (at_keyword) @name) @definition

; Import statements. tree-sitter-css exposes a `string_content` child, so the
; name is unquoted (`other.css`).
(import_statement
  (string_value
    (string_content) @name)) @definition

; Charset declarations
(charset_statement
  (string_value
    (string_content) @name)) @definition

; Property declarations, including custom properties (`--name`), as in
; `symbols.scm`.
(declaration
  (property_name) @name) @field
