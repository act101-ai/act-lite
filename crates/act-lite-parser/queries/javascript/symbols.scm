; Function declarations
(function_declaration
  name: (identifier) @name) @definition

; Generator functions
(generator_function_declaration
  name: (identifier) @name) @definition

; Named function expressions
(function_expression
  name: (identifier) @name) @definition

; Generator function expressions
(generator_function
  name: (identifier) @name) @definition

; Arrow functions assigned to variables
(variable_declarator
  name: (identifier) @name
  value: (arrow_function)) @definition

; Regular variable declarations
(variable_declarator
  name: (identifier) @name) @definition

; Class declarations
(class_declaration
  name: (identifier) @name) @definition

; Method definitions
(method_definition
  name: (property_identifier) @name) @definition

; Field definitions (class fields)
(field_definition
  property: (property_identifier) @name) @definition

; Object property shorthand
(shorthand_property_identifier) @name @definition

; Object property
(pair
  key: (property_identifier) @name) @definition

; Function parameters
(formal_parameters
  (identifier) @name) @definition

; Destructuring patterns
(object_pattern
  (shorthand_property_identifier_pattern) @name) @definition

(array_pattern
  (identifier) @name) @definition
