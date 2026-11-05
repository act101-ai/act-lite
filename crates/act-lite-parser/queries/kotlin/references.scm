; All simple identifiers
(simple_identifier) @reference

; Type identifiers
(type_identifier) @reference

; User types in type references
(user_type
  (type_identifier) @reference)

; Nullable types
(nullable_type
  (user_type
    (type_identifier) @reference))

; Navigation expressions (property access)
(navigation_expression
  (simple_identifier) @reference)

; Call expressions
(call_expression
  (simple_identifier) @reference)

; Constructor invocations
(constructor_invocation
  (user_type
    (type_identifier) @reference))

; Annotation usages
(annotation
  (user_type
    (type_identifier) @reference))

; Import references
(import_header
  (identifier) @reference)

; Super type references
(delegation_specifier
  (user_type
    (type_identifier) @reference))

; Type arguments
(type_arguments
  (type_projection
    (user_type
      (type_identifier) @reference)))
