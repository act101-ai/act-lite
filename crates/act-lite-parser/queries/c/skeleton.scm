; Preprocessor directives
(preproc_include) @include
(preproc_def
  name: (identifier) @name) @macro
(preproc_function_def
  name: (identifier) @name) @macro

; Function declarations
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @function

; Function declarations (prototypes)
(declaration
  declarator: (function_declarator
    declarator: (identifier) @name)) @function

; Struct definitions
(struct_specifier
  name: (type_identifier) @name
  body: (field_declaration_list)) @struct

; Union definitions
(union_specifier
  name: (type_identifier) @name
  body: (field_declaration_list)) @union

; Enum definitions
(enum_specifier
  name: (type_identifier) @name) @enum

; Typedef declarations
(type_definition
  declarator: (type_identifier) @name) @typedef

; Variable declarations (anchored to translation_unit to exclude locals)

; Global variable declarations with initializer
(translation_unit
  (declaration
    declarator: (init_declarator
      declarator: (identifier) @name)) @variable)

; Global variable declarations without initializer
(translation_unit
  (declaration
    declarator: (identifier) @name) @variable)

; Global pointer declarations without initializer
(translation_unit
  (declaration
    declarator: (pointer_declarator
      declarator: (identifier) @name)) @variable)

; Static variable declarations
(translation_unit
  (declaration
    (storage_class_specifier)
    declarator: (init_declarator
      declarator: (identifier) @name)) @variable)

; Struct and union fields
(field_declaration_list
  (field_declaration
    declarator: (field_identifier) @name) @field)
