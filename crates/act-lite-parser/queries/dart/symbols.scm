; Top-level function declarations
; NOTE: tree-sitter-dart 0.0.4 uses "lambda_expression" for named top-level functions.
(lambda_expression
  (function_signature
    (identifier) @name)) @definition

; Class declarations
(class_definition
  (identifier) @name) @definition

; Mixin declarations
(mixin_declaration
  (identifier) @name) @definition

; Extension declarations
(extension_declaration
  (identifier) @name) @definition

; Enum declarations
(enum_declaration
  (identifier) @name) @definition

; Enum constants
(enum_constant
  (identifier) @name) @definition

; Formal parameter declarations
(formal_parameter
  (identifier) @name) @definition

(type_alias
  (type_identifier) @name
  .
  [(comment) (documentation_comment)]*
  .
  "=") @definition

(type_alias
  (type_identifier) @name
  .
  [(comment) (documentation_comment)]*
  .
  (type_parameters)
  .
  [(comment) (documentation_comment)]*
  .
  "=") @definition

(type_alias
  (type_identifier) @name
  .
  [(comment) (documentation_comment)]*
  .
  (formal_parameter_list)) @definition

(type_alias
  (type_identifier) @name
  .
  [(comment) (documentation_comment)]*
  .
  (type_parameters)
  .
  [(comment) (documentation_comment)]*
  .
  (formal_parameter_list)) @definition


; Instance methods, static methods, getters, setters
; (class_member_definition with a function_signature inside method_signature)
(class_member_definition
  (method_signature
    (function_signature
      (identifier) @name))) @definition

; Named constructors: ClassName.factoryName(...)
; The second identifier after the "." is the factory name.
(class_member_definition
  (method_signature
    (constructor_signature
      (identifier)
      "."
      (identifier) @name))) @definition

; Primary constructors: ClassName(...)
(class_member_definition
  (declaration
    (constructor_signature
      (identifier) @name))) @definition

; Class fields: final String name; or String name;
; Structure: class_member_definition -> declaration -> initialized_identifier_list -> initialized_identifier -> identifier
(class_member_definition
  (declaration
    (initialized_identifier_list
      (initialized_identifier
        (identifier) @name)))) @definition

; Local variable declarations
; Structure: local_variable_declaration -> initialized_variable_definition (with name field)
(local_variable_declaration
  (initialized_variable_definition
    name: (identifier) @name)) @definition
