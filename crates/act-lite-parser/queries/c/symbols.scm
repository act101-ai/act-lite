; Function definitions
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition

; Function prototypes
(declaration
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition

; Parameters
(parameter_declaration
  declarator: (identifier) @name) @definition

(parameter_declaration
  declarator: (pointer_declarator
    declarator: (identifier) @name)) @definition

(parameter_declaration
  declarator: (array_declarator
    declarator: (identifier) @name)) @definition

; Struct definitions
(struct_specifier
  name: (type_identifier) @name) @definition

; Struct fields
(field_declaration
  declarator: (field_identifier) @name) @definition

; Union definitions
(union_specifier
  name: (type_identifier) @name) @definition

; Enum definitions
(enum_specifier
  name: (type_identifier) @name) @definition

; Enum values
(enumerator
  name: (identifier) @name) @definition

; Typedef definitions
(type_definition
  declarator: (type_identifier) @name) @definition

; Variable declarations with initializer
(init_declarator
  declarator: (identifier) @name) @definition

(init_declarator
  declarator: (pointer_declarator
    declarator: (identifier) @name)) @definition

; Variable declarations without initializer (bare: int x;)
(declaration
  declarator: (identifier) @name) @definition

(declaration
  declarator: (pointer_declarator
    declarator: (identifier) @name)) @definition

; Macro definitions
(preproc_def
  name: (identifier) @name) @definition

(preproc_function_def
  name: (identifier) @name) @definition

; For loop variables
(for_statement
  initializer: (declaration
    declarator: (init_declarator
      declarator: (identifier) @name))) @definition
