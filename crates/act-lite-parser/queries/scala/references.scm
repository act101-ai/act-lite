; Call sites — direct function calls
(call_expression
  function: (identifier) @reference)

; Call sites — method calls via field access
(call_expression
  function: (field_expression
    field: (identifier) @reference))

; Right-hand side of value bindings (val bar = something — capture "something")
(val_definition
  value: (identifier) @reference)

; Right-hand side of variable bindings
(var_definition
  value: (identifier) @reference)

; Argument positions in call expressions
(arguments
  (identifier) @reference)

; Field access (object.field — capture the field name)
(field_expression
  field: (identifier) @reference)

; Interpolated string references
(interpolation
  (identifier) @reference)

; Extends clauses — the type being extended is a reference
(extends_clause
  (type_identifier) @reference)

; Type arguments (List[Int] — Int is a reference)
(type_arguments
  (type_identifier) @reference)

; Import path segments
(import_declaration
  path: (identifier) @reference)

(namespace_selectors
  (identifier) @reference)

; Generic type references (List[Int] — the outer List)
(generic_type
  (type_identifier) @reference)

; Type annotations on val/var/parameter (the type is a reference)
(val_definition
  type: (type_identifier) @reference)

(var_definition
  type: (type_identifier) @reference)

(parameter
  type: (type_identifier) @reference)

(class_parameter
  type: (type_identifier) @reference)

; Declaring names of functions and types
(function_definition
  name: (identifier) @reference)

(class_definition
  name: (identifier) @reference)

(object_definition
  name: (identifier) @reference)

(trait_definition
  name: (identifier) @reference)

(val_definition
  pattern: (identifier) @reference)
