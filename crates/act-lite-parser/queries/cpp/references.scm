; All identifiers
(identifier) @reference

; Type identifiers
(type_identifier) @reference

; Namespace identifiers
(namespace_identifier) @reference

; Qualified identifiers
(qualified_identifier
  name: (identifier) @reference)

; Template types
(template_type
  name: (type_identifier) @reference)

; Field access
(field_expression
  field: (field_identifier) @reference)

; Function calls
(call_expression
  function: (identifier) @reference)

(call_expression
  function: (qualified_identifier
    name: (identifier) @reference))

; Template function calls
(call_expression
  function: (template_function
    name: (identifier) @reference))

; Constructor calls
(new_expression
  type: (type_identifier) @reference)

; Cast expressions
(cast_expression
  type: (type_descriptor
    type: (type_identifier) @reference))

; Static cast, dynamic cast, etc.
(template_function
  name: (identifier) @reference
  arguments: (template_argument_list))

; Using declarations
(using_declaration
  (qualified_identifier
    name: (identifier) @reference))

; Base class references
(base_class_clause
  (type_identifier) @reference)

; Scope resolution
(qualified_identifier
  scope: (namespace_identifier) @reference)

; Lambda captures
(lambda_capture_specifier
  (identifier) @reference)

; Operator name references (qualified calls like obj.operator==) (REF-CPP-2)
(qualified_identifier
  name: (operator_name) @reference)
