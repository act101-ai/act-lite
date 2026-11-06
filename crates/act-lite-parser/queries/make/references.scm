; Target and prerequisite words
(targets
  (word) @reference)

(prerequisites
  (word) @reference)

; Variable uses: $(NAME) and ${NAME}
(variable_reference
  (word) @reference)

; Assignment and define names
(variable_assignment
  name: (word) @reference)

(shell_assignment
  name: (word) @reference)

(define_directive
  name: (word) @reference)
