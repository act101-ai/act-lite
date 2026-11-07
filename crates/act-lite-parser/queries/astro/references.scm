; Astro component reference sites in the outer document, by HTML's
; conventions: href, class, id and data-* values, and end-tag names.

(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "href")
  (attribute_value) @reference)

(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "href")
  (quoted_attribute_value
    (attribute_value) @reference))

(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "class")
  (attribute_value) @reference)

(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "class")
  (quoted_attribute_value
    (attribute_value) @reference))

(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "id")
  (attribute_value) @reference)

(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "id")
  (quoted_attribute_value
    (attribute_value) @reference))

(attribute
  (attribute_name) @attr_name
  (#match? @attr_name "^data-")
  (attribute_value) @reference)

(attribute
  (attribute_name) @attr_name
  (#match? @attr_name "^data-")
  (quoted_attribute_value
    (attribute_value) @reference))

(end_tag
  (tag_name) @reference)
