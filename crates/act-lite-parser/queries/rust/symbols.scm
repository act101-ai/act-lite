; Capture all symbol definitions

; Function definitions
(function_item
  name: (identifier) @name) @definition

; Struct definitions
(struct_item
  name: (type_identifier) @name) @definition

; Union definitions
(union_item
  name: (type_identifier) @name) @definition

; Struct fields
(field_declaration
  name: (field_identifier) @name) @definition

; Enum definitions
(enum_item
  name: (type_identifier) @name) @definition

; Enum variants
(enum_variant
  name: (identifier) @name) @definition

; Trait definitions
(trait_item
  name: (type_identifier) @name) @definition

; Trait methods (signature only)
(function_signature_item
  name: (identifier) @name) @definition

; Impl blocks
(impl_item
  type: (type_identifier) @name) @definition

; Methods within impl blocks
(impl_item
  body: (declaration_list
    (function_item
      name: (identifier) @name) @definition))

; Type aliases
(type_item
  name: (type_identifier) @name) @definition

; Module definitions
(mod_item
  name: (identifier) @name) @definition

; Const definitions
(const_item
  name: (identifier) @name) @definition

; Static definitions
(static_item
  name: (identifier) @name) @definition

; Let bindings
(let_declaration
  pattern: (identifier) @name) @definition

; Function parameters
(parameter
  pattern: (identifier) @name) @definition

; Macro definitions
(macro_definition
  name: (identifier) @name) @definition

; Associated types in traits
(associated_type
  name: (type_identifier) @name) @definition
