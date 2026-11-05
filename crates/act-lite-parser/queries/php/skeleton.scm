; Namespace declaration
(namespace_definition
  name: (namespace_name) @name) @namespace

; Use/import declarations
(namespace_use_declaration) @import

; Class declarations
(class_declaration
  name: (name) @name) @class

; Interface declarations
(interface_declaration
  name: (name) @name) @interface

; Trait declarations
(trait_declaration
  name: (name) @name) @trait

; Enum declarations
(enum_declaration
  name: (name) @name) @enum

; Function declarations
(function_definition
  name: (name) @name) @function

(const_declaration
  (const_element
    . (name) @name)) @constant

; Method declarations (enables navigation to methods within large classes)
(method_declaration
  name: (name) @name) @method
