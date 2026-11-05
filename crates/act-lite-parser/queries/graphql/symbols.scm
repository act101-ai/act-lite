; Object types and their extensions
(object_type_definition
  (name) @name) @definition.class

(object_type_extension
  (name) @name) @definition.class

; Interfaces
(interface_type_definition
  (name) @name) @definition.interface

; Input objects
(input_object_type_definition
  (name) @name) @definition.struct

; Enums
(enum_type_definition
  (name) @name) @definition.enum

; Unions
(union_type_definition
  (name) @name) @definition.union

; Scalars
(scalar_type_definition
  (name) @name) @definition.type_alias

; Directive definitions
(directive_definition
  (name) @name) @definition.attribute

; Named operations
(operation_definition
  (name) @name) @definition.function

; Fragments
(fragment_definition
  (fragment_name
    (name) @name)) @definition.constant

; Fields of types and interfaces
(field_definition
  (name) @name) @definition.field

; Fields of input objects
(input_fields_definition
  (input_value_definition
    (name) @name) @definition.field)

; Enum values
(enum_value_definition
  (enum_value
    (name) @name)) @definition.enum_member
