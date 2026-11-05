; Module declarations
(module
  name: (constant) @name) @definition

; Nested modules
(module
  name: (scope_resolution
    name: (constant) @name)) @definition

; Class declarations
(class
  name: (constant) @name) @definition

; Class with inheritance
(class
  name: (constant) @name
  superclass: (superclass
    (constant) @superclass_name)) @definition

; Method declarations
(method
  name: (identifier) @name) @definition

; Operator methods
(method
  name: (operator) @name) @definition

; Singleton methods (class methods)
(singleton_method
  name: (identifier) @name) @definition

; Block parameters
(block_parameters
  (identifier) @name) @definition

; Method parameters
(method_parameters
  (identifier) @name) @definition

; Optional parameters
(optional_parameter
  name: (identifier) @name) @definition

; Keyword parameters
(keyword_parameter
  name: (identifier) @name) @definition

; Splat parameters
(splat_parameter
  name: (identifier) @name) @definition

; Hash splat parameters
(hash_splat_parameter
  name: (identifier) @name) @definition

; Block parameter
(block_parameter
  name: (identifier) @name) @definition

; Assignment (local variables)
(assignment
  left: (identifier) @name) @definition

; Assignment (instance variables)
(assignment
  left: (instance_variable) @name) @definition

; Assignment (class variables)
(assignment
  left: (class_variable) @name) @definition

; Constant assignment
(assignment
  left: (constant) @name) @definition


; Rescue variable
(rescue
  (exception_variable
    (identifier) @name)) @definition

; For loop variable
(for
  (identifier) @name) @definition

; Pattern matching variable bindings (Ruby 3.0+)
(in_clause
  pattern: (identifier) @name) @definition

; Begin block local variables
(begin_block
  (assignment
    left: (identifier) @name)) @definition
