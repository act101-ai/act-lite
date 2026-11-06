; Object types and their extensions
(object_type_definition
  (name) @name) @class

(object_type_extension
  (name) @name) @class

; Interfaces and their extensions
(interface_type_definition
  (name) @name) @interface

(interface_type_extension
  (name) @name) @interface

; Input objects and their extensions
(input_object_type_definition
  (name) @name) @struct

(input_object_type_extension
  (name) @name) @struct

; Enums and their extensions
(enum_type_definition
  (name) @name) @enum

(enum_type_extension
  (name) @name) @enum

; Unions and their extensions
(union_type_definition
  (name) @name) @union

(union_type_extension
  (name) @name) @union

; Scalars and their extensions
(scalar_type_definition
  (name) @name) @type_alias

(scalar_type_extension
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
