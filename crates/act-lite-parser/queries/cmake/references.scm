; Command names
(normal_command
  (identifier) @reference)

; ${name}
(normal_var
  (variable) @reference)

; Unquoted arguments
(argument
  (unquoted_argument) @reference)

; Quoted and bracket arguments
(argument
  (quoted_argument
    (quoted_element) @reference))

(argument
  (bracket_argument
    (bracket_argument_content) @reference))
