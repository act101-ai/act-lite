; Functions: an assignment of a function definition, at any depth
(binary_operator
  lhs: [(identifier) @name (string (string_content) @name)]
  operator: ["<-" "=" "<<-"]
  rhs: (function_definition)) @function

; Other file-level assignments; the name is the assigned side
(program
  (binary_operator
    lhs: [(identifier) @name (string (string_content) @name)]
    operator: ["<-" "=" "<<-"]
    rhs: (_) @_value
    (#not-match? @_value "^(function[\\s(]|\\\\)")) @variable)

(program
  (binary_operator
    operator: ["->" "->>"]
    rhs: [(identifier) @name (string (string_content) @name)]) @variable)

; library(x) and require(x)
(call
  function: (identifier) @_call
  arguments: (arguments . (argument value: (_) @name))
  (#match? @_call "^(library|require)$")) @import

; source("f.R")
(call
  function: (identifier) @_call
  arguments: (arguments . (argument value: (_) @name))
  (#eq? @_call "source")) @include
