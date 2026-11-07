; Vue component symbols: the outer document. Blocks are modules at their
; start tag, template elements classes at their tag name, literal `id` and
; `class` values fields, as for HTML. The script and style regions are
; reported from their own trees.

; Top-level blocks: `<template>` and custom blocks (`<i18n>`, `<docs>`, any
; other top-level element), at their start tag.
(document
  (template_element
    (start_tag) @name @definition.module))

(document
  (element
    [(start_tag) (self_closing_tag)] @name @definition.module))

; Template structure: an element whose parent is a template or an element.
; A `<template>` nested in a template is an element named `template`.
(template_element
  [(element
     [(start_tag (tag_name) @name)
      (self_closing_tag (tag_name) @name)] @definition.class)
   (template_element
     (start_tag (tag_name) @name) @definition.class)])

(element
  [(element
     [(start_tag (tag_name) @name)
      (self_closing_tag (tag_name) @name)] @definition.class)
   (template_element
     (start_tag (tag_name) @name) @definition.class)])

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
           (quoted_attribute_value (attribute_value) @name)]) @definition.field)
      (self_closing_tag
        (attribute
          (attribute_name) @attr_name
          (#any-of? @attr_name "id" "class")
          [(attribute_value) @name
           (quoted_attribute_value (attribute_value) @name)]) @definition.field)])
   (template_element
     (start_tag
       (attribute
         (attribute_name) @attr_name
         (#any-of? @attr_name "id" "class")
         [(attribute_value) @name
          (quoted_attribute_value (attribute_value) @name)]) @definition.field))])

(element
  [(element
     [(start_tag
        (attribute
          (attribute_name) @attr_name
          (#any-of? @attr_name "id" "class")
          [(attribute_value) @name
           (quoted_attribute_value (attribute_value) @name)]) @definition.field)
      (self_closing_tag
        (attribute
          (attribute_name) @attr_name
          (#any-of? @attr_name "id" "class")
          [(attribute_value) @name
           (quoted_attribute_value (attribute_value) @name)]) @definition.field)])
   (template_element
     (start_tag
       (attribute
         (attribute_name) @attr_name
         (#any-of? @attr_name "id" "class")
         [(attribute_value) @name
          (quoted_attribute_value (attribute_value) @name)]) @definition.field))])
