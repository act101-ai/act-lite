(package_declaration
  [(scoped_identifier) @name
   (identifier) @name]) @package

; Import statements
(import_declaration) @import

; Class declarations
(class_declaration
  name: (identifier) @name) @class

; Interface declarations
(interface_declaration
  name: (identifier) @name) @interface

; Enum declarations
(enum_declaration
  name: (identifier) @name) @enum

; Record declarations (Java 16+)
(record_declaration
  name: (identifier) @name) @record

; Annotation type declarations
(annotation_type_declaration
  name: (identifier) @name) @annotation

; Field declarations
(field_declaration
  declarator: (variable_declarator
    name: (identifier) @name)) @field

; Method declarations
(method_declaration
  name: (identifier) @name) @method

; Constructor declarations
(constructor_declaration
  name: (identifier) @name) @constructor

; Enum constants
(enum_constant
  name: (identifier) @name) @enum_constant

(static_initializer
  ("static") @name) @static_initializer

(module_declaration
  name: [(scoped_identifier) @name
         (identifier) @name]) @module
