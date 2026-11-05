; Markdown symbols query - matches all defining occurrences

; ATX Headings
(atx_heading
  (heading_content) @name) @definition

; Setext Headings
(setext_heading
  (heading_content) @name) @definition

; Link reference labels
(link_reference_definition
  (link_label) @name) @definition
