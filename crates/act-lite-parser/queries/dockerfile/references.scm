(expansion
  (variable) @reference)
(shell_fragment) @reference
(heredoc_line) @reference

; Stage alias: FROM image AS alias
(from_instruction
  as: (image_alias) @reference)
; Stage use: COPY --from=alias
(copy_instruction
  (param) @reference)
; Stage use: FROM alias (the classifier keeps only names declared by AS)
(from_instruction
  (image_spec
    name: (image_name) @reference))
