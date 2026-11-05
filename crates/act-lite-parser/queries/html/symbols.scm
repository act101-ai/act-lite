;; HTML symbols query — named elements

;; Capture element start tags as definitions
(start_tag
  (tag_name) @name) @definition

;; Capture self-closing tags
(self_closing_tag
  (tag_name) @name) @definition

;; Capture id attributes for rename operations
(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "id")
  (attribute_value) @name) @definition

(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "id")
  (quoted_attribute_value
    (attribute_value) @name)) @definition

;; Capture class attribute values
(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "class")
  (attribute_value) @name) @definition

(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "class")
  (quoted_attribute_value
    (attribute_value) @name)) @definition
