; Bash skeleton query
;
; Extracts the high-level structure of a Bash source file, including:
; - Function definitions (both `name() { }` and `function name { }` forms)
; - Top-level variable assignments
; - Source / dot-source directives (treated as imports)
;
; Capture names:
;   @name     - Identifier name of the declaration
;   @function - Function definition node
;   @variable - Variable assignment node
;   @import   - Source/dot-source directive node

; Function definitions: greet() { echo hello; }
; Function definitions: function greet { echo hello; }
; Both forms parse to function_definition with a word child for the name.
(function_definition
  name: (word) @name) @function

; Top-level variable assignments: MY_VAR=hello
(variable_assignment
  name: (variable_name) @name) @variable

; Source directives: source ./lib.sh
(command
  name: (command_name
    (word) @_src)
  (#eq? @_src "source")) @import

; Dot-source directives: . ./lib.sh
(command
  name: (command_name
    (word) @_dot)
  (#eq? @_dot ".")) @import
