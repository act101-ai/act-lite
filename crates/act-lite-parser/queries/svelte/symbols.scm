; Svelte component symbols: the markup structure, with HTML's conventions.
; The script and style blocks and their symbols come from the regions.

; Each element at its tag name.
(element
  [(start_tag (tag_name) @name)
   (self_closing_tag (tag_name) @name)]) @definition.class

; Literal `id` and `class` values; one symbol per class. A value that holds
; an expression is not literal and not reported.
(element
  (start_tag
    (attribute
      (attribute_name) @attr_name
      (#any-of? @attr_name "id" "class")
      (attribute_value) @name
      (#not-match? @name "[{]")) @definition.field))

(element
  (start_tag
    (attribute
      (attribute_name) @attr_name
      (#any-of? @attr_name "id" "class")
      (quoted_attribute_value
        (attribute_value) @name
        (#not-match? @name "[{]"))) @definition.field))

(element
  (self_closing_tag
    (attribute
      (attribute_name) @attr_name
      (#any-of? @attr_name "id" "class")
      (attribute_value) @name
      (#not-match? @name "[{]")) @definition.field))

(element
  (self_closing_tag
    (attribute
      (attribute_name) @attr_name
      (#any-of? @attr_name "id" "class")
      (quoted_attribute_value
        (attribute_value) @name
        (#not-match? @name "[{]"))) @definition.field))

; `{#snippet name(...)}` at its name.
(snippet_statement
  (snippet_start
    (snippet_name) @name)) @definition.function
