; Function definitions
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition

(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier
      name: (identifier) @name))) @definition

; Method definitions (qualified)
(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier
      scope: (namespace_identifier)
      name: (identifier) @name))) @definition

; Constructor definitions
(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier) @name)) @definition

; Destructor definitions
(function_definition
  declarator: (function_declarator
    declarator: (destructor_name) @name)) @definition

; Class definitions
(class_specifier
  name: (type_identifier) @name) @definition

; Struct definitions
(struct_specifier
  name: (type_identifier) @name) @definition

; Field declarations
(field_declaration
  declarator: (field_identifier) @name) @definition

; Methods defined inside class/struct bodies
(function_definition
  declarator: (function_declarator
    declarator: (field_identifier) @name)) @definition

; Namespace definitions
(namespace_definition
  name: (namespace_identifier) @name) @definition

; Template parameters
(type_parameter_declaration
  (type_identifier) @name) @definition

; Template function definitions
(template_declaration
  (function_definition
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition

; Alias declarations
(alias_declaration
  name: (type_identifier) @name) @definition

; Enum definitions
(enum_specifier
  name: (type_identifier) @name) @definition

; Enum values
(enumerator
  name: (identifier) @name) @definition

; Variable declarations
(init_declarator
  declarator: (identifier) @name) @definition

; Lambda captures
(lambda_capture_specifier
  (identifier) @name) @definition

; Parameters
(parameter_declaration
  declarator: (identifier) @name) @definition

(parameter_declaration
  declarator: (reference_declarator
    (identifier) @name)) @definition

; Structured bindings (C++17)
(structured_binding_declarator
  (identifier) @name) @definition

; Concept definitions (C++20)
(concept_definition
  name: (identifier) @name) @definition

; Macro definitions (SYM-CPP-1)
(preproc_def
  name: (identifier) @name) @definition

(preproc_function_def
  name: (identifier) @name) @definition

; Operator overloads (SYM-CPP-2)
(function_definition
  declarator: (function_declarator
    declarator: (operator_name) @name)) @definition
