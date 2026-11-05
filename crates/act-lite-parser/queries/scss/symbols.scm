; Symbol definitions in SCSS
; Only using node types that exist in tree-sitter-scss

; Variable declarations ($variable) - parsed as declaration with property_name starting with $
(declaration
  (property_name) @name) @definition

; Mixin declarations (@mixin name)
(mixin_statement
  (identifier) @name) @definition

; Function declarations (@function name)
(function_statement
  (identifier) @name) @definition

; Class selectors (.classname)
(class_selector
  (class_name
    (identifier) @name)) @definition

; Tag names (type selectors)
(tag_name) @name

; Keyframe selectors (0%, 50%, 100%, from, to)
(keyframe_block
  (from) @name)

(keyframe_block
  (to) @name)

; Rule sets; the name keeps its sigils (`.btn`), as in the skeleton query.
(rule_set
  (selectors) @name) @definition
