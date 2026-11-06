; Vue component skeleton declarations: none yet. The one pattern below
; compiles against the grammar and matches no tag, because a tag name is
; never empty text.
((tag_name) @name @element
  (#eq? @name ""))
