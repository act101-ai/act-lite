; Import declarations
(import_declaration) @import

; Top-level functions
(function_declaration
  name: (simple_identifier) @name) @function

; Class declarations (tree-sitter-swift uses class_declaration for class/struct/enum/actor/extension)
; Use #match? on the captured node text to distinguish the declaration keyword.

; Struct declarations
((class_declaration
  name: (type_identifier) @name) @struct
 (#match? @struct "^\\s*struct "))

; Enum declarations
((class_declaration
  name: (type_identifier) @name) @enum
 (#match? @enum "^\\s*enum "))

; Class declarations (class keyword)
((class_declaration
  name: (type_identifier) @name) @class
 (#match? @class "^\\s*class "))

; Actor declarations (map to class kind)
((class_declaration
  name: (type_identifier) @name) @class
 (#match? @class "^\\s*actor "))

; Extension declarations (map to class kind)
((class_declaration
  name: (type_identifier) @name) @class
 (#match? @class "^\\s*extension "))

; Protocol declarations
(protocol_declaration
  name: (type_identifier) @name) @interface

; Type aliases
(typealias_declaration
  name: (type_identifier) @name) @type_alias

; Variable/constant declarations (top-level)
(property_declaration
  (pattern
    (simple_identifier) @name)) @variable

; Protocol associated type
(associatedtype_declaration
  name: (type_identifier) @name) @type_alias

(init_declaration
  name: "init" @name) @function
