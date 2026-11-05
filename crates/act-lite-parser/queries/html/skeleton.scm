
(element
  [(start_tag (tag_name) @name)
   (self_closing_tag (tag_name) @name)]) @element

; id attribute values
(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "id")
  (attribute_value) @name) @field

(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "id")
  (quoted_attribute_value
    (attribute_value) @name)) @field

; class attribute values; one declaration per class
(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "class")
  (attribute_value) @name) @field

(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "class")
  (quoted_attribute_value
    (attribute_value) @name)) @field
