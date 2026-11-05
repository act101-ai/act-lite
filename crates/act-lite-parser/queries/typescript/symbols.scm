; Capture all symbol definitions

(function_declaration
  name: (identifier) @name) @definition

; Generator functions
(generator_function_declaration
  name: (identifier) @name) @definition

(class_declaration
  name: (type_identifier) @name) @definition

(interface_declaration
  name: (type_identifier) @name) @definition

(type_alias_declaration
  name: (type_identifier) @name) @definition

(enum_declaration
  name: (identifier) @name) @definition

; Variable declarations (covers arrow functions and regular assignments)
(variable_declarator
  name: (identifier) @name) @definition

(method_definition
  name: (property_identifier) @name) @definition

(property_signature
  name: (property_identifier) @name) @definition

(public_field_definition
  name: (property_identifier) @name) @definition

; Namespace names
(internal_module
  name: (identifier) @name) @definition

; Object property shorthand
(shorthand_property_identifier) @name @definition

; Object property
(pair
  key: (property_identifier) @name) @definition

; Destructuring patterns
(object_pattern
  (shorthand_property_identifier_pattern) @name) @definition

(array_pattern
  (identifier) @name) @definition
