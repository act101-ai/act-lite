; Capture identifier usages for reference tracking

; All identifiers
(identifier) @reference

; Type identifiers
(type_identifier) @reference

; Field identifiers
(field_identifier) @reference

; Scoped identifiers (path segments)
(scoped_identifier
  name: (identifier) @reference)

; Field expressions
(field_expression
  field: (field_identifier) @reference)

; Method calls
(call_expression
  function: (field_expression
    field: (field_identifier) @reference))
