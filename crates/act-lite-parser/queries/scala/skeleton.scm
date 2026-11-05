; Package declarations
(package_clause
  (package_identifier) @name) @package

; Import declarations
(import_declaration) @import

; Function definitions
(function_definition
  name: (identifier) @name) @function

; Class definitions
(class_definition
  name: (identifier) @name) @class

; Object definitions
(object_definition
  name: (identifier) @name) @class

; Trait definitions
(trait_definition
  name: (identifier) @name) @interface

; Type alias definitions
(type_definition
  name: (type_identifier) @name) @type_alias

; Value definitions (top-level)
(val_definition
  pattern: (identifier) @name) @variable

; Variable definitions (top-level)
(var_definition
  pattern: (identifier) @name) @variable

; Method definitions inside template bodies (classes, objects, traits)
(template_body
  (function_definition
    name: (identifier) @name) @method)

; Enum definitions (Scala 3)
(enum_definition
  name: (identifier) @name) @enum
