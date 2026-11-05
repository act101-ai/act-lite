; Package clause
(package_clause
  (package_identifier) @name) @package

; Import declarations
(import_declaration) @import

; Function declarations
(function_declaration
  name: (identifier) @name) @function

; Method declarations
(method_declaration
  name: (field_identifier) @name) @method

; Non-struct/interface type declarations. Tree-sitter queries cannot negate a
; child node type, so enumerate the grammar's closed non-specialized `_type`
; variants; the two specialized rules below own struct/interface specs.
(type_declaration
  (type_spec
    name: (type_identifier) @name
    type: [
      (array_type)
      (channel_type)
      (function_type)
      (generic_type)
      (map_type)
      (negated_type)
      (parenthesized_type)
      (pointer_type)
      (qualified_type)
      (slice_type)
      (type_identifier)
    ]) @type_alias)

; Struct types specifically
(type_declaration
  (type_spec
    name: (type_identifier) @name
    type: (struct_type)) @struct)

; Interface types specifically
(type_declaration
  (type_spec
    name: (type_identifier) @name
    type: (interface_type)) @interface)

; Const declarations
(const_declaration
  (const_spec
    name: (identifier) @name) @constant)

; Var declarations
(var_declaration
  (var_spec
    name: (identifier) @name) @variable)

; Struct fields
(field_declaration
  name: (field_identifier) @name) @field
