; References in call positions: foo(), Engine()
(call_expression
  (simple_identifier) @reference)

; Navigation expression base: engine.method() — captures "engine"
(navigation_expression
  (simple_identifier) @reference)

; Navigation suffix member names: engine.start(), engine.rpm — captures "start", "rpm"
; NOTE: tree-sitter-swift does not represent function_declaration names as navigation_suffix,
; so this does not capture declaration sites. Verify on grammar upgrades.
(navigation_suffix
  (simple_identifier) @reference)

; Type references in type annotations: var x: Int, var x: MyType
(user_type
  (type_identifier) @reference)

; Declaring names of functions and types
(function_declaration
  name: (simple_identifier) @reference)

(class_declaration
  name: (type_identifier) @reference)

(protocol_declaration
  name: (type_identifier) @reference)
