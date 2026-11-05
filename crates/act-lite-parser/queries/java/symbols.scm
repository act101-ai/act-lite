; Class declarations
(class_declaration
  name: (identifier) @name) @definition

; Interface declarations
(interface_declaration
  name: (identifier) @name) @definition

; Enum declarations
(enum_declaration
  name: (identifier) @name) @definition

; Record declarations
(record_declaration
  name: (identifier) @name) @definition

; Annotation type declarations
(annotation_type_declaration
  name: (identifier) @name) @definition

; Method declarations
(method_declaration
  name: (identifier) @name) @definition

; Constructor declarations
(constructor_declaration
  name: (identifier) @name) @definition

; Compact constructor declarations (for records)
(compact_constructor_declaration
  name: (identifier) @name) @definition

; Field declarations
(field_declaration
  declarator: (variable_declarator
    name: (identifier) @name)) @definition

; Constant declarations
(constant_declaration
  declarator: (variable_declarator
    name: (identifier) @name)) @definition

; Local variable declarations
(local_variable_declaration
  declarator: (variable_declarator
    name: (identifier) @name)) @definition

; Formal parameters
(formal_parameter
  name: (identifier) @name) @definition

; Catch clause parameters
(catch_formal_parameter
  name: (identifier) @name) @definition

; Lambda parameters - inferred
(inferred_parameters
  (identifier) @name) @definition

; Lambda expression with explicit parameter
(lambda_expression
  parameters: (identifier) @name) @definition

; Enum constants
(enum_constant
  name: (identifier) @name) @definition

; Type parameters
(type_parameter
  (type_identifier) @name) @definition

; Pattern variable (Java 16+ pattern matching)
(type_pattern
  (identifier) @name) @definition

; Enhanced for loop variable
(enhanced_for_statement
  name: (identifier) @name) @definition
