; Symbol references in SCSS
; Only using node types that exist in tree-sitter-scss 1.0

; Variable/function references - identifiers in expressions
(identifier) @reference

; Mixin calls - captured as call_expression
(call_expression) @reference

; String values (for module paths)
(string_value) @reference
