; Variable references — anchored to read contexts only.
; Do NOT use a top-level (variable_name) capture; it captures every variable
; in the tree (including definition sites) and produces false positives.

; Right-hand side of assignment
(assignment_expression
  right: (variable_name) @reference)

; Function/method call arguments
(argument
  (variable_name) @reference)

; Return statements
(return_statement
  (variable_name) @reference)

; Echo / print statements
(echo_statement
  (variable_name) @reference)

; Conditions (parenthesized expression in if/while/for)
(parenthesized_expression
  (variable_name) @reference)

; Array element values
(array_element_initializer
  (variable_name) @reference)

; Function calls
(function_call_expression
  function: (name) @reference)

; Method calls
(member_call_expression
  name: (name) @reference)

; Nullsafe method calls ($obj?->method())
(nullsafe_member_call_expression
  name: (name) @reference)

; Nullsafe property access ($obj?->prop)
(nullsafe_member_access_expression
  name: (name) @reference)

; Static method calls
(scoped_call_expression
  name: (name) @reference)

; Class instantiation
(object_creation_expression
  (name) @reference)

; Static property access
(scoped_property_access_expression
  name: (variable_name) @reference)

; Namespace names
(namespace_name) @reference

; Qualified names
(qualified_name
  (namespace_name) @reference)

; Use declarations
(namespace_use_clause
  (qualified_name
    (namespace_name) @reference))

; Type declarations in parameters
(named_type
  (name) @reference)

(named_type
  (qualified_name
    (namespace_name) @reference))

; Property access
(member_access_expression
  name: (name) @reference)

; Implements/extends
(class_interface_clause
  (name) @reference)

(base_clause
  (name) @reference)

; Trait use
(use_declaration
  (name) @reference)

; Attribute references
(attribute
  (qualified_name
    (namespace_name) @reference))

(attribute
  (name) @reference)

; Union/intersection types
(union_type
  (named_type
    (name) @reference))

(intersection_type
  (named_type
    (name) @reference))

; Declaring names of types, functions and methods
(class_declaration
  name: (name) @reference)

(interface_declaration
  name: (name) @reference)

(trait_declaration
  name: (name) @reference)

(function_definition
  name: (name) @reference)

(method_declaration
  name: (name) @reference)

(enum_declaration
  name: (name) @reference)
