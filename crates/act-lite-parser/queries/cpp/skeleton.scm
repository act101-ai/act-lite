; All C skeleton patterns apply to C++ as well
; Include preprocessor directives
(preproc_include) @include
(preproc_def
  name: (identifier) @name) @macro

; Namespace definitions
(namespace_definition
  name: (namespace_identifier) @name) @namespace

; Namespace alias definitions
(namespace_alias_definition
  name: (namespace_identifier) @name) @namespace

; Class definitions
(class_specifier
  name: (type_identifier) @name) @class

; Struct definitions (in C++ context)
(struct_specifier
  name: (type_identifier) @name) @struct

; Union definitions
(union_specifier
  name: (type_identifier) @name
  body: (field_declaration_list)) @union

; Template declarations
(template_declaration
  (class_specifier
    name: (type_identifier) @name)) @class

(template_declaration
  (struct_specifier
    name: (type_identifier) @name)) @struct

(template_declaration
  (function_definition
    declarator: (function_declarator
      declarator: (identifier) @name))) @function

; Function definitions
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @function

(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier
      name: (identifier) @name))) @function

(function_definition
  declarator: (function_declarator
    declarator: (destructor_name) @name)) @function

; Enum class (scoped enums)
(enum_specifier
  name: (type_identifier) @name) @enum

; Type aliases
(alias_declaration
  name: (type_identifier) @name) @typedef

(using_declaration
  [(qualified_identifier) @name
   (identifier) @name]) @using

; Concept definitions (C++20)
(concept_definition
  name: (identifier) @name) @concept

; Global variable declarations (anchored to translation_unit to exclude locals)

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

; Class, struct and union fields
(field_declaration_list
  (field_declaration
    declarator: (field_identifier) @name) @field)

; Methods defined inside class/struct bodies
(function_definition
  declarator: (function_declarator
    declarator: (field_identifier) @name)) @method
