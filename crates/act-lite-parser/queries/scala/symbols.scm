; Function definitions
(function_definition
  name: (identifier) @name) @definition

; Class definitions
(class_definition
  name: (identifier) @name) @definition

; Object definitions
(object_definition
  name: (identifier) @name) @definition

; Trait definitions
(trait_definition
  name: (identifier) @name) @definition

; Type definitions
(type_definition
  name: (type_identifier) @name) @definition

; Value definitions
(val_definition
  pattern: (identifier) @name) @definition

; Variable definitions
(var_definition
  pattern: (identifier) @name) @definition

; Class parameters — explicit name: field to avoid capturing type identifiers
(class_parameter
  name: (identifier) @name) @definition

; Function parameters — explicit name: field to avoid capturing type identifiers
(parameter
  name: (identifier) @name) @definition

; Type parameters (direct children of type_parameters)
(type_parameters
  (identifier) @name) @definition

; Pattern bindings in case clauses
(case_clause
  (identifier) @name) @definition

; For comprehension bindings
(enumerator
  (identifier) @name) @definition

; Enum definitions (Scala 3)
(enum_definition
  name: (identifier) @name) @definition

; Enum cases (Scala 3)
(simple_enum_case
  name: (identifier) @name) @definition

(full_enum_case
  name: (identifier) @name) @definition
