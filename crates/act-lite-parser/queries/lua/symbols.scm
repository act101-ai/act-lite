; Lua symbols query — all definitions

; Named function declaration (global and local)
(function_declaration
  name: (identifier) @name) @definition

; Global assignment
(assignment_statement
  (variable_list (identifier) @name)) @definition

; Local declaration
(variable_declaration
  (assignment_statement
    (variable_list (identifier) @name))) @definition
