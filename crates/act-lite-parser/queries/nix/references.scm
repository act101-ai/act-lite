; Variable uses
(variable_expression
  name: (identifier) @reference)

; Attribute paths: binding names and the paths of selects, whole
(attrpath) @reference

; inherit lists
(inherited_attrs
  attr: (identifier) @reference)

; Parameters
(function_expression
  universal: (identifier) @reference)

(formal
  name: (identifier) @reference)
