; Capture top-level declarations for file skeleton

; Functions
(function_item
  name: (identifier) @name) @function

; Structs
(struct_item
  name: (type_identifier) @name) @class

; Unions
(union_item
  name: (type_identifier) @name) @class

; Enums
(enum_item
  name: (type_identifier) @name) @enum

; Traits
(trait_item
  name: (type_identifier) @name) @interface

; Impl blocks
(impl_item
  type: (type_identifier) @name) @impl

; Impl blocks with trait
(impl_item
  trait: (type_identifier) @trait_name
  type: (type_identifier) @name) @impl

; Type aliases
(type_item
  name: (type_identifier) @name) @type_alias

; Modules
(mod_item
  name: (identifier) @name) @module

; Constants
(const_item
  name: (identifier) @name) @constant

; Statics
(static_item
  name: (identifier) @name) @constant

; Use declarations
(use_declaration) @import

; Macro definitions
(macro_definition
  name: (identifier) @name) @macro

; Struct fields
(field_declaration
  name: (field_identifier) @name) @field
