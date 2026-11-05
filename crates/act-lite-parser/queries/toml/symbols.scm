; TOML symbols query — all named definitions
;
; Captures table headers and top-level key names as @definition

; Table headers ([table])
(table
  (bare_key) @name) @definition

(table
  (quoted_key) @name) @definition

(table
  (dotted_key) @name) @definition

; Array-of-tables headers ([[table]])
(table_array_element
  (bare_key) @name) @definition

(table_array_element
  (quoted_key) @name) @definition

(table_array_element
  (dotted_key) @name) @definition

; Top-level key-value pairs
(document
  (pair
    (bare_key) @name) @definition)

(document
  (pair
    (quoted_key) @name) @definition)

(document
  (pair
    (dotted_key) @name) @definition)

; Key-value pairs under a table header
(table
  (pair
    (bare_key) @name) @definition)

(table
  (pair
    (quoted_key) @name) @definition)

(table
  (pair
    (dotted_key) @name) @definition)

(table_array_element
  (pair
    (bare_key) @name) @definition)

(table_array_element
  (pair
    (quoted_key) @name) @definition)

(table_array_element
  (pair
    (dotted_key) @name) @definition)
