; Scoped type identifiers (qualified names)
(scoped_type_identifier
  (type_identifier) @reference)

; Scoped identifiers
(scoped_identifier
  (identifier) @reference)

; Method invocation targets
(method_invocation
  name: (identifier) @reference)

; Method invocation on object
(method_invocation
  object: (identifier) @reference)

; Field access
(field_access
  field: (identifier) @reference)

(field_access
  object: (identifier) @reference)

; Object creation
(object_creation_expression
  type: (type_identifier) @reference)

; Generic type arguments
(type_arguments
  (type_identifier) @reference)

; Annotation usages
(annotation
  name: (identifier) @reference)

(marker_annotation
  name: (identifier) @reference)

; Super and this
(super) @reference
(this) @reference

; Class literals
(class_literal
  (type_identifier) @reference)

; Declaring names of types and methods
(class_declaration
  name: (identifier) @reference)

(interface_declaration
  name: (identifier) @reference)

(enum_declaration
  name: (identifier) @reference)

(record_declaration
  name: (identifier) @reference)

(method_declaration
  name: (identifier) @reference)

(annotation_type_declaration
  name: (identifier) @reference)
