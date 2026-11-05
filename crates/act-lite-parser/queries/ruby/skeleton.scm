; Require statements
(call
  method: (identifier) @require_method (#match? @require_method "^require")
  arguments: (argument_list)) @import

; Module declarations
(module
  name: (constant) @name) @module

; Class declarations
(class
  name: (constant) @name) @class

; Singleton class (class << self)
(singleton_class
  value: (self) @name) @class

; Method declarations (top-level or nested)
(method
  name: (identifier) @name) @function

; Singleton method (def self.method_name)
(singleton_method
  name: (identifier) @name) @function

; Constant assignments (top-level)
(assignment
  left: (constant) @name) @constant

; Struct definitions
(assignment
  left: (constant) @name
  right: (call
    receiver: (constant) @struct_type (#eq? @struct_type "Struct")
    method: (identifier) @new_method (#eq? @new_method "new"))) @class

(call
  method: (identifier) @accessor_type
  (#match? @accessor_type "^attr_(reader|writer|accessor)$")
  arguments: (argument_list (simple_symbol) @name)) @property

(call
  method: (identifier) @name (#eq? @name "module_function")) @function
