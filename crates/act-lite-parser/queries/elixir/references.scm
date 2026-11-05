; Module aliases
(alias) @reference

; Atoms
(atom) @reference

; Function calls
(call
  target: (identifier) @reference)

; Qualified calls (Module.function)
(call
  target: (dot
    left: (alias) @reference
    right: (identifier) @reference))

; Pipe operator
(binary_operator
  operator: "|>"
  right: (identifier) @reference)

(binary_operator
  operator: "|>"
  right: (call
    target: (identifier) @reference))

; Module attribute references
(unary_operator
  operator: "@"
  operand: (identifier) @reference)

; Capture operator
(unary_operator
  operator: "&"
  operand: (identifier) @reference)

; Directive references
(call
  target: (identifier) @directive (#match? @directive "^(alias|import|use|require)$")
  (arguments (alias) @reference))

; Behaviour reference
(unary_operator
  operator: "@"
  operand: (call
    target: (identifier) @behaviour (#match? @behaviour "^behaviou?r$")
    (arguments (alias) @reference)))

; String interpolation
(interpolation
  (identifier) @reference)
