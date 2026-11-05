; Table definitions — name is inside object_reference → identifier
(create_table
  (object_reference
    (identifier) @name)) @definition

; View definitions — name is inside object_reference → identifier
(create_view
  (object_reference
    (identifier) @name)) @definition

; Index definitions — name is a direct identifier child (no object_reference wrapper)
(create_index
  (identifier) @name) @definition

; Function/procedure definitions — name is inside object_reference → identifier
(create_function
  (object_reference
    (identifier) @name)) @definition

; CTE definitions — alias is a direct identifier child
(cte
  (identifier) @name) @definition

; Columns of CREATE TABLE
(column_definition
  name: (identifier) @name) @definition
