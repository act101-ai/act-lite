; Vue component reference sites in the outer document, with HTML's
; conventions: `href`, `class`, `id` and `data-*` values, end-tag names.

;; Capture id references from href attributes (like href="#section")
(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "href")
  (attribute_value) @reference)

;; href twin: the quoted shape
(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "href")
  (quoted_attribute_value
    (attribute_value) @reference))

;; Capture class attribute values as references (for rename_class operations)
(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "class")
  (attribute_value) @reference)

;; class twin: the quoted shape
(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "class")
  (quoted_attribute_value
    (attribute_value) @reference))

;; Capture id attribute references
(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "id")
  (attribute_value) @reference)

;; id twin: the quoted shape
(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "id")
  (quoted_attribute_value
    (attribute_value) @reference))

;; Capture data-* attributes as references
(attribute
  (attribute_name) @attr_name
  (#match? @attr_name "^data-")
  (attribute_value) @reference)

;; data-* twin: the quoted shape
(attribute
  (attribute_name) @attr_name
  (#match? @attr_name "^data-")
  (quoted_attribute_value
    (attribute_value) @reference))

(end_tag
  (tag_name) @reference)

