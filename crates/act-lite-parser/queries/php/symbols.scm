; Class declarations
(class_declaration
  name: (name) @name) @definition

; Interface declarations
(interface_declaration
  name: (name) @name) @definition

; Trait declarations
(trait_declaration
  name: (name) @name) @definition

; Enum declarations
(enum_declaration
  name: (name) @name) @definition

; Enum cases
(enum_case
  name: (name) @name) @definition

; Function definitions
(function_definition
  name: (name) @name) @definition

; Method declarations
(method_declaration
  name: (name) @name) @definition

; Property declarations
(property_declaration
  (property_element
    (variable_name
      (name) @name))) @definition

; Property promotion in constructor
(property_promotion_parameter
  name: (variable_name
    (name) @name)) @definition

(const_declaration
  (const_element
    . (name) @name)) @definition

; Simple parameters
(simple_parameter
  name: (variable_name
    (name) @name)) @definition

; Variadic parameters
(variadic_parameter
  name: (variable_name
    (name) @name)) @definition

; Static variables
(static_variable_declaration
  (variable_name
    (name) @name)) @definition

; Global variables
(global_declaration
  (variable_name
    (name) @name)) @definition

; Variable assignment (simple)
(assignment_expression
  left: (variable_name
    (name) @name)) @definition

; Anonymous function parameters — simple
(anonymous_function
  parameters: (formal_parameters
    (simple_parameter
      name: (variable_name
        (name) @name)))) @definition

; Anonymous function parameters — variadic (...$args)
(anonymous_function
  parameters: (formal_parameters
    (variadic_parameter
      name: (variable_name
        (name) @name)))) @definition

; Arrow function parameters — simple
(arrow_function
  parameters: (formal_parameters
    (simple_parameter
      name: (variable_name
        (name) @name)))) @definition

; Arrow function parameters — variadic (...$args)
(arrow_function
  parameters: (formal_parameters
    (variadic_parameter
      name: (variable_name
        (name) @name)))) @definition

; Namespace
(namespace_definition
  name: (namespace_name) @name) @definition
