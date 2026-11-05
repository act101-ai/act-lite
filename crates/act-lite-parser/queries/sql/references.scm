; FROM clause — table references (SELECT/JOIN)
(from
  (relation
    (object_reference
      (identifier) @reference)))

; FROM clause — direct object_reference (DELETE FROM)
(from
  (object_reference
    (identifier) @reference))

; JOIN references
(join
  (relation
    (object_reference
      (identifier) @reference)))

; UPDATE target table
(update
  (relation
    (object_reference
      (identifier) @reference)))

; INSERT target table
(insert
  (object_reference
    (identifier) @reference))

; Field references — qualified: table.column (object_reference wraps identifier)
(field
  (object_reference
    (identifier) @reference))

; Field references — bare column in WHERE/SET/SELECT (identifier directly under field)
; Discovery: tree-sitter-sequel represents `user_id` in WHERE and `id` in SELECT list
; as (field (identifier)) — no object_reference wrapper. This covers SQL-16 and SQL-17.
(field
  (identifier) @reference)

; Function calls
(invocation
  (object_reference
    (identifier) @reference))

; Declaring names of tables and views (the name, not the schema)
(create_table
  (object_reference
    name: (identifier) @reference))

(create_view
  (object_reference
    name: (identifier) @reference))
