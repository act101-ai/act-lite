; All identifiers
(identifier) @reference

; Type identifiers
(type_identifier) @reference

; Field identifiers
(field_identifier) @reference

; Package identifiers
(package_identifier) @reference

; Selector expressions
(selector_expression
  field: (field_identifier) @reference)

; Qualified types
(qualified_type
  package: (package_identifier) @reference
  name: (type_identifier) @reference)

(import_spec
  path: (interpreted_string_literal) @reference)

