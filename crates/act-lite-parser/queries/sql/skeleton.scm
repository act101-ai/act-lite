
; DML statements — named by their own keyword
(select
  (keyword_select) @name) @function

(insert
  (keyword_insert) @name) @function

(update
  (keyword_update) @name) @function

(delete
  (keyword_delete) @name) @function

; CREATE TABLE
(create_table
  (object_reference
    name: (identifier) @name)) @class

; CREATE VIEW
(create_view
  (object_reference
    name: (identifier) @name)) @class

; ALTER TABLE — schema mutations
(alter_table
  (object_reference
    name: (identifier) @name)) @class

; CREATE INDEX — the grammar labels the *index's* own name `column:`
(create_index
  column: (identifier) @name) @variable

; CREATE FUNCTION — anchored to the first object_reference, which is the
; function's own name
(create_function
  .
  (keyword_create)
  .
  (keyword_function)
  .
  (object_reference
    name: (identifier) @name)) @function

; CREATE TRIGGER — a trigger holds three object_references (its own name, the
; table it fires on, and the procedure it executes); the anchors take the first
(create_trigger
  .
  (keyword_create)
  .
  (keyword_trigger)
  .
  (object_reference
    name: (identifier) @name)) @function

(cte
  .
  (identifier) @name) @function

; Column definitions (in CREATE TABLE)
(column_definition
  name: (identifier) @name) @property

; Constraints
(constraint
  name: (identifier) @name) @property

; DROP TABLE — schema evolution operations
(drop_table
  (object_reference
    name: (identifier) @name)) @class

; DROP VIEW — verified to exist in tree-sitter-sequel
(drop_view
  (object_reference
    name: (identifier) @name)) @class

; DROP INDEX — verified to exist in tree-sitter-sequel
(drop_index
  name: (identifier) @name) @variable

