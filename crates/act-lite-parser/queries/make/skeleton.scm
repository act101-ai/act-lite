; Targets, one entry per target word; GNU special targets are skipped
(rule
  (targets
    (word) @name
    (#not-match? @name "^\\.[A-Z_]+$"))) @function

; Variable assignments, except target-specific ones
(variable_assignment
  !target_or_pattern
  name: (word) @name) @variable

(shell_assignment
  name: (word) @name) @variable

(define_directive
  name: (word) @name) @variable

; Include directives
(include_directive
  filenames: (_) @name) @include
