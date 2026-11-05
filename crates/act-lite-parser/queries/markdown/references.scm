;; Markdown references query - matches link usages

;; Inline links: [text](url)
(link
  (link_text) @text
  (link_destination) @url) @reference

;; Reference-style links: [text][label]
(link
  (link_text) @text
  (link_label) @label) @reference

;; Link reference definitions
(link_reference_definition
  (link_label) @label
  (link_destination) @url) @reference

;; Headings: the declaring name
(atx_heading
  (heading_content) @reference)

(setext_heading
  (heading_content) @reference)
