; Symbol definitions in CSS

; Class selectors (.classname)
(class_selector
  (class_name) @name) @definition

; ID selectors (#id)
(id_selector
  (id_name) @name) @definition

; Type selectors (element names like div, p, span)
(selectors
  (tag_name) @name) @definition

; Property declarations, including custom properties (`--name`)
(declaration
  (property_name) @name) @definition

; Keyframe selectors (0%, 50%, 100%)
(keyframe_block
  (from) @name)

(keyframe_block
  (to) @name)
