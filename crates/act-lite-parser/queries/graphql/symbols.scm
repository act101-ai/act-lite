; Object types and their extensions
(object_type_definition
  (name) @name) @definition.class

(object_type_extension
  (name) @name) @definition.class

; Interfaces and their extensions
(interface_type_definition
  (name) @name) @definition.interface

(interface_type_extension
  (name) @name) @definition.interface

; Input objects and their extensions
(input_object_type_definition
  (name) @name) @definition.struct

(input_object_type_extension
  (name) @name) @definition.struct

; Enums and their extensions
(enum_type_definition
  (name) @name) @definition.enum

(enum_type_extension
  (name) @name) @definition.enum

; Unions and their extensions
(union_type_definition
  (name) @name) @definition.union

(union_type_extension
  (name) @name) @definition.union

; Scalars and their extensions
(scalar_type_definition
  (name) @name) @definition.type_alias

(scalar_type_extension
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
