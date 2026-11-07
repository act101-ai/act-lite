; Astro component skeleton: the outer document. Region blocks and the
; declarations inside regions come from the region model; this file gives
; the template structure, by HTML's conventions, and the block of each
; data-block <script>, which is not a region.

; Template elements, named by their tag as written.
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

; A data block: a <script> whose first type attribute, with leading and
; trailing ASCII whitespace removed, is not empty, not module and not a
; JavaScript MIME type (ASCII case-insensitive). Its block is named by its
; start tag.
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
       (quoted_attribute_value (attribute_value) @_value)])) @name
  (#not-match? @_before "^type([\\t\\n\\f\\r =]|$)")
  (#not-match? @_value "^[\\t\\n\\f\\r ]*((?i-u:module|application/ecmascript|application/javascript|application/x-ecmascript|application/x-javascript|text/ecmascript|text/javascript|text/javascript1\\.[0-5]|text/jscript|text/livescript|text/x-ecmascript|text/x-javascript)[\\t\\n\\f\\r ]*)?$")) @module

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
       (quoted_attribute_value (attribute_value) @_value)])) @name
  (#not-match? @_before "^type([\\t\\n\\f\\r =]|$)")
  (#not-match? @_value "^[\\t\\n\\f\\r ]*((?i-u:module|application/ecmascript|application/javascript|application/x-ecmascript|application/x-javascript|text/ecmascript|text/javascript|text/javascript1\\.[0-5]|text/jscript|text/livescript|text/x-ecmascript|text/x-javascript)[\\t\\n\\f\\r ]*)?$")) @module
