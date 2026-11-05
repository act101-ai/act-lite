; Functions
(function_declaration
  name: (identifier) @name) @function

; Generator functions
(generator_function_declaration
  name: (identifier) @name) @function

; Classes
(class_declaration
  name: (identifier) @name) @class

; Top-level variables (const, let, var)
(lexical_declaration
  (variable_declarator
    name: (identifier) @name)) @variable

(variable_declaration
  (variable_declarator
    name: (identifier) @name)) @variable

; Methods
(method_definition
  name: (property_identifier) @name) @method

; Imports
(import_statement) @import

; ESM exports
(export_statement) @export

; CommonJS exports (module.exports = ...)
(expression_statement
  (assignment_expression
    left: (member_expression
      object: (identifier) @_module
      property: (property_identifier) @_exports)
    (#eq? @_module "module")
    (#eq? @_exports "exports"))) @export
