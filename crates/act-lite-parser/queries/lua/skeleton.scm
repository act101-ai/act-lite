; Lua skeleton query — top-level structure

; Named function declaration: function foo() end
; Also matches: local function foo() end (both parse as function_declaration)
(function_declaration
  name: (identifier) @name) @function

; Class-like table definition: ClassName = {} or ClassName = { fields }
; Lua OOP convention: uppercase-named table assigned a table_constructor
(assignment_statement
  (variable_list (identifier) @name)
  (expression_list (table_constructor))) @class

; Global assignment: x = 42 (non-table-constructor rhs)
(assignment_statement
  (variable_list (identifier) @name)) @variable

; Local declaration: local x = ...
(variable_declaration
  (assignment_statement
    (variable_list (identifier) @name))) @variable
