; Table headers ([table])
(table
  (bare_key) @name) @module

(table
  (quoted_key) @name) @module

(table
  (dotted_key) @name) @module

; Array-of-tables headers ([[table]])
(table_array_element
  (bare_key) @name) @module

(table_array_element
  (quoted_key) @name) @module

(table_array_element
  (dotted_key) @name) @module

; Top-level key-value pairs
(document
  (pair
    (bare_key) @name) @variable)

(document
  (pair
    (quoted_key) @name) @variable)

(document
  (pair
    (dotted_key) @name) @variable)

; Key-value pairs under a table header
(table
  (pair
    (bare_key) @name) @variable)

(table
  (pair
    (quoted_key) @name) @variable)

(table
  (pair
    (dotted_key) @name) @variable)

(table_array_element
  (pair
    (bare_key) @name) @variable)

(table_array_element
  (pair
    (quoted_key) @name) @variable)

(table_array_element
  (pair
    (dotted_key) @name) @variable)
