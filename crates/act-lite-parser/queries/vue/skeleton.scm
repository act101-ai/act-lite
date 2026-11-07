; Vue component skeleton: the outer document. The script and style
; regions are reported from their own trees.

; Top-level blocks: `<template>` and custom blocks (`<i18n>`, `<docs>`, any
; other top-level element), named by their start tag.
(document
  (template_element
    (start_tag) @name) @module)

(document
  (element
    [(start_tag) (self_closing_tag)] @name) @module)

; Template structure: an element whose parent is a template or an element.
; A `<template>` nested in a template is an element named `template`.
(template_element
  [(element
     [(start_tag (tag_name) @name)
      (self_closing_tag (tag_name) @name)])
   (template_element
     (start_tag (tag_name) @name))] @element)

(element
  [(element
     [(start_tag (tag_name) @name)
      (self_closing_tag (tag_name) @name)])
   (template_element
     (start_tag (tag_name) @name))] @element)

; The literal `id` value and each token of a literal `class` value of a
; template element, with HTML's conventions. Dynamic attributes (`:class`,
; `v-bind:id`) are directives, not attributes.
(template_element
  [(element
     [(start_tag
        (attribute
          (attribute_name) @attr_name
          (#any-of? @attr_name "id" "class")
          [(attribute_value) @name
           (quoted_attribute_value (attribute_value) @name)]) @field)
      (self_closing_tag
        (attribute
          (attribute_name) @attr_name
          (#any-of? @attr_name "id" "class")
          [(attribute_value) @name
           (quoted_attribute_value (attribute_value) @name)]) @field)])
   (template_element
     (start_tag
       (attribute
         (attribute_name) @attr_name
         (#any-of? @attr_name "id" "class")
         [(attribute_value) @name
          (quoted_attribute_value (attribute_value) @name)]) @field))])

(element
  [(element
     [(start_tag
        (attribute
          (attribute_name) @attr_name
          (#any-of? @attr_name "id" "class")
          [(attribute_value) @name
           (quoted_attribute_value (attribute_value) @name)]) @field)
      (self_closing_tag
        (attribute
          (attribute_name) @attr_name
          (#any-of? @attr_name "id" "class")
          [(attribute_value) @name
           (quoted_attribute_value (attribute_value) @name)]) @field)])
   (template_element
     (start_tag
       (attribute
         (attribute_name) @attr_name
         (#any-of? @attr_name "id" "class")
         [(attribute_value) @name
          (quoted_attribute_value (attribute_value) @name)]) @field))])
