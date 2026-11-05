; Bash references query
;
; Finds all identifier references in a Bash source file, including:
; - Simple variable expansions: $VAR
; - Brace variable expansions: ${VAR}, ${VAR:-default}
; - Command names (function calls and program invocations)
;
; Capture names:
;   @reference - A variable or command name reference node

; Simple variable expansions: $VAR, $i
(simple_expansion
  (variable_name) @reference)

; Brace expansions: ${VAR}, ${VAR:-default}, ${VAR:+value}
(expansion
  (variable_name) @reference)

; Command names (function calls, program invocations): deploy_app, echo, git
(command
  name: (command_name
    (word) @reference))

; Function definitions: the declaring name
(function_definition
  name: (word) @reference)
