; Bash symbols query
;
; Finds all symbol definitions in a Bash source file, including:
; - Function definitions (both syntax forms)
; - Variable assignments (including array and declare forms)
; - For loop iteration variables
;
; Capture names:
;   @name       - Identifier name being defined
;   @definition - The complete definition node

; Function definitions: greet() { } and function greet { }
(function_definition
  name: (word) @name) @definition

; Simple variable assignments: MY_VAR=hello
(variable_assignment
  name: (variable_name) @name) @definition

; For loop iteration variables: for i in ...; do ...; done
(for_statement
  variable: (variable_name) @name) @definition
