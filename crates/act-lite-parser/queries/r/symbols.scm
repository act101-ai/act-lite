; Functions: an assignment of a function definition, at any depth
(binary_operator
  lhs: [(identifier) @name (string (string_content) @name)]
  operator: ["<-" "=" "<<-"]
  rhs: (function_definition)) @definition.function

; Other file-level assignments; the name is the assigned side
(program
  (binary_operator
    lhs: [(identifier) @name (string (string_content) @name)]
    operator: ["<-" "=" "<<-"]
    rhs: (_) @_value
    (#not-match? @_value "^(function[\\s(]|\\\\)")) @definition.variable)

(program
  (binary_operator
    operator: ["->" "->>"]
    rhs: [(identifier) @name (string (string_content) @name)]) @definition.variable)

; Parameters, `...` included
(parameter
  name: [(identifier) (dots)] @name) @definition.parameter
