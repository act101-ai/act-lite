; All identifiers
(identifier) @reference

; Type identifiers
(type_identifier) @reference

; Field identifiers
(field_identifier) @reference

; Function calls
(call_expression
  function: (identifier) @reference)

; Field access
(field_expression
  field: (field_identifier) @reference)

; Pointer field access
(field_expression
  argument: (identifier) @reference)

; Array subscript
(subscript_expression
  argument: (identifier) @reference)

; Sizeof expressions
(sizeof_expression
  type: (type_descriptor
    type: (type_identifier) @reference))

; Cast expressions
(cast_expression
  type: (type_descriptor
    type: (type_identifier) @reference))

