; Astro component symbols: the outer document. Region blocks and the
; symbols inside regions come from the region model; this file gives the
; template structure, by HTML's conventions, and the block of each
; data-block <script> at its start tag.

; Template elements, at the tag name.
(element
  (start_tag (tag_name) @name)) @definition.class

(element
  (self_closing_tag (tag_name) @name)) @definition.class

; id attribute values
(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "id")
  (attribute_value) @name) @definition.field

(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "id")
  (quoted_attribute_value
    (attribute_value) @name)) @definition.field

; class attribute values; one symbol per class
(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "class")
  (attribute_value) @name) @definition.field

(attribute
  (attribute_name) @attr_name
  (#eq? @attr_name "class")
  (quoted_attribute_value
    (attribute_value) @name)) @definition.field

; A data block (see skeleton.scm): a module at its start tag.
(script_element
  (start_tag
    (tag_name)
    .
    (attribute)* @_before
    .
    (attribute
      (attribute_name) @_type
      (#eq? @_type "type")
      [(attribute_value) @_value
       (quoted_attribute_value (attribute_value) @_value)])) @name @definition.module
  (#not-match? @_before "^type([\\t\\n\\f\\r =]|$)")
  (#not-match? @_value "^[\\t\\n\\f\\r ]*((?i-u:module|application/ecmascript|application/javascript|application/x-ecmascript|application/x-javascript|text/ecmascript|text/javascript|text/javascript1\\.[0-5]|text/jscript|text/livescript|text/x-ecmascript|text/x-javascript)[\\t\\n\\f\\r ]*)?$"))

(script_element
  (self_closing_tag
    (tag_name)
    .
    (attribute)* @_before
    .
    (attribute
      (attribute_name) @_type
      (#eq? @_type "type")
      [(attribute_value) @_value
       (quoted_attribute_value (attribute_value) @_value)])) @name @definition.module
  (#not-match? @_before "^type([\\t\\n\\f\\r =]|$)")
  (#not-match? @_value "^[\\t\\n\\f\\r ]*((?i-u:module|application/ecmascript|application/javascript|application/x-ecmascript|application/x-javascript|text/ecmascript|text/javascript|text/javascript1\\.[0-5]|text/jscript|text/livescript|text/x-ecmascript|text/x-javascript)[\\t\\n\\f\\r ]*)?$"))
