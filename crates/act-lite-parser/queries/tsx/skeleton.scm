; Capture top-level declarations for file skeleton

(function_declaration
  name: (identifier) @name) @function

(generator_function_declaration
  name: (identifier) @name) @function

(class_declaration
  name: (type_identifier) @name) @class

(interface_declaration
  name: (type_identifier) @name) @interface

(type_alias_declaration
  name: (type_identifier) @name) @type_alias

(enum_declaration
  name: (identifier) @name) @enum

(method_definition
  name: (property_identifier) @name) @method

; Variable declarations (const, let, var)
(lexical_declaration
  (variable_declarator
    name: (identifier) @name)) @variable

; Namespace / module declarations
(internal_module
  name: (identifier) @name) @namespace

; Abstract classes
(abstract_class_declaration
  name: (type_identifier) @name) @class

(import_statement) @import

(export_statement) @export
