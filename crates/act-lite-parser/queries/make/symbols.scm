; Targets, one entry per target word; GNU special targets are skipped
(rule
  (targets
    (word) @name
    (#not-match? @name "^\\.[A-Z_]+$"))) @definition.function

; Variable assignments, except target-specific ones
(variable_assignment
  !target_or_pattern
  name: (word) @name) @definition.variable

(shell_assignment
  name: (word) @name) @definition.variable

(define_directive
  name: (word) @name) @definition.variable
