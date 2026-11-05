; Python skeleton query
;
; Extracts the high-level structure of a Python source file, including:
; - Module docstrings
; - Import statements
; - Function definitions
; - Class definitions
; - Top-level variable assignments
; - Type aliases (Python 3.12+)
;
; Capture names:
;   @name     - Identifier name of the declaration
;   @function - Function/method declaration node
;   @class    - Class declaration node
;   @variable - Module-level variable assignment node
;   @import   - Import statement node
;   @docstring - Module-level docstring node
;   @type     - Type alias declaration node

(module
  (expression_statement
    (string
      (string_content) @name)) @docstring)

; Import statements
(import_statement) @import

(import_from_statement) @import

; Function definitions (including async - async is a modifier on function_definition)
(function_definition
  name: (identifier) @name) @function

; Decorated definitions
(decorated_definition
  definition: (function_definition
    name: (identifier) @name)) @function

; Class definitions
(class_definition
  name: (identifier) @name) @class

(decorated_definition
  definition: (class_definition
    name: (identifier) @name)) @class

; Top-level assignments (module-level variables/constants)
(module
  (expression_statement
    (assignment
      left: (identifier) @name))) @variable

; Type alias (Python 3.12+)
(type_alias_statement
  left: (type) @name) @type_alias
