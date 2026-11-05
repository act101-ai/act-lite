; Python symbols query
;
; Finds all symbol definitions in a Python source file, including:
; - Functions and methods
; - Classes
; - Variables (assignments, augmented assignments)
; - For loop variables
; - With statement variables
; - Exception handler variables
; - Function parameters (including *args, **kwargs)
; - Comprehension variables
; - Import aliases
;
; Capture names:
;   @name       - Identifier name being defined
;   @definition - The complete definition node

; Function definitions
(function_definition
  name: (identifier) @name) @definition

; Lambda (when assigned to a variable)
(assignment
  left: (identifier) @name
  right: (lambda)) @definition

; Class definitions
(class_definition
  name: (identifier) @name) @definition

; Variable assignments
(assignment
  left: (identifier) @name) @definition

; Augmented assignments
(augmented_assignment
  left: (identifier) @name) @definition

; For loop variables
(for_statement
  left: (identifier) @name) @definition

; With statement variables
(with_statement
  (with_clause
    (with_item
      (as_pattern
        (as_pattern_target
          (identifier) @name))))) @definition

; Exception handler variable
(except_clause
  (as_pattern
    (as_pattern_target
      (identifier) @name))) @definition

; Function parameters
(parameters
  (identifier) @name) @definition

(parameters
  (typed_parameter
    (identifier) @name)) @definition

(parameters
  (default_parameter
    name: (identifier) @name)) @definition

(parameters
  (typed_default_parameter
    name: (identifier) @name)) @definition

; *args and **kwargs
(parameters
  (list_splat_pattern
    (identifier) @name)) @definition

(parameters
  (dictionary_splat_pattern
    (identifier) @name)) @definition

; Comprehension variables
(list_comprehension
  (for_in_clause
    left: (identifier) @name)) @definition

(dictionary_comprehension
  (for_in_clause
    left: (identifier) @name)) @definition

(set_comprehension
  (for_in_clause
    left: (identifier) @name)) @definition

(generator_expression
  (for_in_clause
    left: (identifier) @name)) @definition

; Import aliases
(import_statement
  name: (aliased_import
    alias: (identifier) @name)) @definition

(import_from_statement
  name: (aliased_import
    alias: (identifier) @name)) @definition
