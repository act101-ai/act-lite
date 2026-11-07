; Svelte component skeleton: the markup structure, with HTML's conventions.
; The script and style blocks and their declarations come from the regions.
; A <script> or <style> inside markup is not reported.

; Each element, named by its tag as written (components and `svelte:*`
; elements included).
(element
  [(start_tag (tag_name) @name)
   (self_closing_tag (tag_name) @name)]) @element

; Literal `id` and `class` values; one declaration per class. A value that
; holds an expression (`class="a {x}"`) is not literal and not reported.
(element
  (start_tag
    (attribute
      (attribute_name) @attr_name
      (#any-of? @attr_name "id" "class")
      (attribute_value) @name
      (#not-match? @name "[{]")) @field))

(element
  (start_tag
    (attribute
      (attribute_name) @attr_name
      (#any-of? @attr_name "id" "class")
      (quoted_attribute_value
        (attribute_value) @name
        (#not-match? @name "[{]"))) @field))

(element
  (self_closing_tag
    (attribute
      (attribute_name) @attr_name
      (#any-of? @attr_name "id" "class")
      (attribute_value) @name
      (#not-match? @name "[{]")) @field))

(element
  (self_closing_tag
    (attribute
      (attribute_name) @attr_name
      (#any-of? @attr_name "id" "class")
      (quoted_attribute_value
        (attribute_value) @name
        (#not-match? @name "[{]"))) @field))

; `{#snippet name(...)}` declares a function.
(snippet_statement
  (snippet_start
    (snippet_name) @name)) @function
