; Symbol references in CSS

; References to class selectors
(class_selector
  (class_name
    (identifier) @reference))

; References to ID selectors
(id_selector
  (id_name) @reference)

; References to type selectors (element names like div, p, span)
(selectors
  (tag_name) @reference)

; References to custom properties in var() functions
(call_expression
  (function_name) @func
  (#eq? @func "var")
  (arguments
    (identifier) @reference))

; References to custom properties in plain values
(plain_value) @reference
  (#match? @reference "^--")
