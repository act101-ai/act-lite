;; Identifier references (usages) in HCL/Terraform
;; Captures all references to variables, locals, resources, etc.

;; Simple identifiers (variable references, local references, etc.)
(identifier) @reference

;; Function call identifiers
(function_call
  (identifier) @reference)

;; Declaring names: the only label of variable/output/module blocks, and the
;; last (name) label of resource/data blocks. Type labels and the labels of
;; blocks that declare nothing (provider, terraform) are not declarations.
(block
  .
  (identifier) @_kind
  .
  (string_lit
    (template_literal) @reference)
  .
  (block_start)
  (#any-of? @_kind "variable" "output" "module"))

(block
  .
  (identifier) @_kind
  .
  (string_lit)
  .
  (string_lit
    (template_literal) @reference)
  .
  (block_start)
  (#any-of? @_kind "resource" "data"))
