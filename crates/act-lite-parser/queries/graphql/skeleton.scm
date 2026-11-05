; Object types and their extensions
(object_type_definition
  (name) @name) @class

(object_type_extension
  (name) @name) @class

; Interfaces
(interface_type_definition
  (name) @name) @interface

; Input objects
(input_object_type_definition
  (name) @name) @struct

; Enums
(enum_type_definition
  (name) @name) @enum

; Unions
(union_type_definition
  (name) @name) @union

; Scalars
(scalar_type_definition
  (name) @name) @type_alias

; Directive definitions
(directive_definition
  (name) @name) @annotation

; Named operations
(operation_definition
  (name) @name) @function

; Fragments
(fragment_definition
  (fragment_name
    (name) @name)) @constant
