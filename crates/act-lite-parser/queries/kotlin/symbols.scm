; Function declarations
(function_declaration
  (simple_identifier) @name) @definition

; Class declarations (all types)
(class_declaration
  (type_identifier) @name) @definition

; Object declarations
(object_declaration
  (type_identifier) @name) @definition

(companion_object
  (type_identifier) @name) @definition

; Property declarations
(property_declaration
  (variable_declaration
    (simple_identifier) @name)) @definition

; Function parameters
(function_value_parameters
  (parameter
    (simple_identifier) @name)) @definition

; Lambda parameters
(lambda_literal
  (lambda_parameters
    (variable_declaration
      (simple_identifier) @name))) @definition

; Type aliases
(type_alias
  (type_identifier) @name) @definition

; Enum entries
(enum_entry
  (simple_identifier) @name) @definition

; Type parameters
(type_parameters
  (type_parameter
    (type_identifier) @name)) @definition

; For loop variable
(for_statement
  (variable_declaration
    (simple_identifier) @name)) @definition

; Catch clause variable
(catch_block
  (simple_identifier) @name) @definition
