; Package declaration
(package_header
  (identifier) @name) @package

; Import declarations
(import_header) @import

; Top-level functions
(function_declaration
  (simple_identifier) @name) @function

; Interfaces (including plain interface; fun interface not supported by tree-sitter-kotlin 0.3.x)
(class_declaration
  "interface"
  (type_identifier) @name) @interface

; Enum classes — match "enum" as the first child (before "class")
(class_declaration
  .
  "enum"
  (type_identifier) @name) @enum

; Classes (plain, data, sealed, open, abstract, inner, value, annotation)
; Uses (modifiers)? . "class" to exclude enum class (which starts with "enum", not "class")
(class_declaration
  (modifiers)?
  .
  "class"
  (type_identifier) @name) @class

; Object declarations (singletons)
(object_declaration
  (type_identifier) @name) @class

(companion_object
  (type_identifier) @name) @class

; Type aliases
(type_alias
  (type_identifier) @name) @type_alias

; Property declarations (top-level)
(property_declaration
  (variable_declaration
    (simple_identifier) @name)) @variable
