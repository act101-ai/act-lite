; Python references query
;
; Finds all identifier references in a Python source file, including:
; - Standalone identifiers
; - Attribute access (e.g., obj.attr)
; - Subscript operations (e.g., arr[key])
; - Function/method calls
; - Decorator names
; - Type annotations
;
; Capture names:
;   @reference - An identifier reference node

; All identifiers (references)
(identifier) @reference

; Attribute access (the attribute name)
(attribute
  attribute: (identifier) @reference)

; Subscript target
(subscript
  value: (identifier) @reference)

; Call function name
(call
  function: (identifier) @reference)

; Method call
(call
  function: (attribute
    object: (identifier) @reference
    attribute: (identifier) @reference))

; Decorator names
(decorator
  (identifier) @reference)

(decorator
  (call
    function: (identifier) @reference))

; Type annotations
(type
  (identifier) @reference)

(generic_type
  (identifier) @reference)

(subscript
  (identifier) @reference)
