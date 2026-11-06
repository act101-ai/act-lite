; Svelte component reference sites in the markup, with HTML's conventions:
; literal `href`, `class`, `id` and `data-*` values, and end-tag names. A
; value that holds an expression is not literal and not a site.

(element
  [(start_tag
     (attribute
       (attribute_name) @attr_name
       (#any-of? @attr_name "href" "class" "id")
       (attribute_value) @reference
       (#not-match? @reference "[{]")))
   (self_closing_tag
     (attribute
       (attribute_name) @attr_name
       (#any-of? @attr_name "href" "class" "id")
       (attribute_value) @reference
       (#not-match? @reference "[{]")))])

(element
  [(start_tag
     (attribute
       (attribute_name) @attr_name
       (#any-of? @attr_name "href" "class" "id")
       (quoted_attribute_value
         (attribute_value) @reference
         (#not-match? @reference "[{]"))))
   (self_closing_tag
     (attribute
       (attribute_name) @attr_name
       (#any-of? @attr_name "href" "class" "id")
       (quoted_attribute_value
         (attribute_value) @reference
         (#not-match? @reference "[{]"))))])

(element
  [(start_tag
     (attribute
       (attribute_name) @attr_name
       (#match? @attr_name "^data-")
       (attribute_value) @reference
       (#not-match? @reference "[{]")))
   (self_closing_tag
     (attribute
       (attribute_name) @attr_name
       (#match? @attr_name "^data-")
       (attribute_value) @reference
       (#not-match? @reference "[{]")))])

(element
  [(start_tag
     (attribute
       (attribute_name) @attr_name
       (#match? @attr_name "^data-")
       (quoted_attribute_value
         (attribute_value) @reference
         (#not-match? @reference "[{]"))))
   (self_closing_tag
     (attribute
       (attribute_name) @attr_name
       (#match? @attr_name "^data-")
       (quoted_attribute_value
         (attribute_value) @reference
         (#not-match? @reference "[{]"))))])

(element
  (end_tag
    (tag_name) @reference))
